mod config;
mod database;
mod error;
mod models;
mod prelude;
mod routes;
mod schema;

use crate::{
  config::{Config, Context},
  database::{cache::CacheManager, DBConnectionManager},
  prelude::*,
  routes::Build,
};
use axum::Extension;
use clap::Parser;
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

extern crate tracing;

pub(crate) const MINA_GOVERNANCE_SERVER: &str = "mina_governance_server";

#[tokio::main]
async fn main() -> Result<()> {
  config::init_tracing();

  let config = Config::parse();
  let cache = CacheManager::build();
  let cors = config::init_cors(&config);

  tracing::info!(
      target: MINA_GOVERNANCE_SERVER,
      "Initializing database connection pools..."
  );

  let conn_manager = DBConnectionManager::get_connections(&config);

  let router = axum::Router::build().layer(
    ServiceBuilder::new()
      .layer(TraceLayer::new_for_http())
      .layer(cors)
      .layer(Extension(Context {
        cache: Arc::new(cache),
        conn_manager: Arc::new(conn_manager),
        network: config.mina_network,
        ledger_storage_path: config.ledger_storage_path,
        bucket_name: config.bucket_name,
      })),
  );

  serve(router.clone(), config.port).await;
  Ok(())
}

async fn serve(router: axum::Router, port: u16) {
  let addr = SocketAddr::from(([0, 0, 0, 0], port));

  tracing::info!(
      target: MINA_GOVERNANCE_SERVER,
      "Started server on {addr} - http://{addr}"
  );

  axum::Server::bind(&addr)
    .serve(router.into_make_service())
    .with_graceful_shutdown(shutdown())
    .await
    .expect("Error: failed to start axum runtime");
}

async fn shutdown() {
  let windows = async {
    signal::ctrl_c()
      .await
      .unwrap_or_else(|_| panic!("Error: failed to install windows shutdown handler"));
  };

  #[cfg(unix)]
  let unix = async {
    signal::unix::signal(signal::unix::SignalKind::terminate())
      .unwrap_or_else(|_| panic!("Error: failed to install unix shutdown handler"))
      .recv()
      .await;
  };

  #[cfg(not(unix))]
  let terminate = std::future::pending::<()>();

  tokio::select! {
      () = windows => {},
      () = unix => {},
  }

  println!("Signal received - starting graceful shutdown...");
}
