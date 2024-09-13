mod archive;
mod cache;
mod config;
mod error;
mod handlers;
mod ledger;
mod prelude;
mod proposals;
mod vote;

use anyhow::Context as AnyhowContext;
use axum::{routing::get, Extension, Router};
use cache::CacheManager;
use clap::Parser;
pub use config::*;
use diesel::{r2d2::ConnectionManager, PgConnection};
pub use ledger::*;
use prelude::*;
pub use proposals::*;
use r2d2::Pool;
use std::{net::SocketAddr, sync::Arc};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
pub use vote::*;

extern crate tracing;

pub const MINA_GOVERNANCE_SERVER: &str = "mina_governance_server";

#[tokio::main]
async fn main() -> Result<()> {
  config::init_tracing();

  let config = Config::parse();
  let cache = CacheManager::build();
  let cors = config::init_cors(&config);
  let manifest = MinaProposalManifest::load().await?;

  tracing::info!(
    target: MINA_GOVERNANCE_SERVER,
    "Initializing database connection pools...",
  );

  let archive_manager = ConnectionManager::<PgConnection>::new(&config.archive_database_url);
  let conn_manager = Pool::builder()
    .test_on_check_out(true)
    .build(archive_manager)
    .unwrap_or_else(|_| panic!("Error: failed to build `archive` connection pool"));

  let router = Router::new()
    .route("/api/info", get(handlers::get_core_api_info))
    .route("/api/proposals", get(handlers::get_mina_proposals))
    .route("/api/proposal/:id", get(handlers::get_mina_proposal))
    .route("/api/proposal/:id/results", get(handlers::get_mina_proposal_result))
    .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()).layer(cors).layer(Extension(Context {
      cache: Arc::new(cache),
      conn_manager: Arc::new(conn_manager),
      network: config.mina_network,
      ledger_storage_path: config.ledger_storage_path,
      bucket_name: config.bucket_name,
      manifest,
    })));

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
    signal::ctrl_c().await.unwrap_or_else(|_| panic!("Error: failed to install windows shutdown handler"));
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
