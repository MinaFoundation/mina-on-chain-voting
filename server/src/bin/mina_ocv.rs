use anyhow::Result;
use axum::{
  debug_handler,
  extract::{Path, State},
  response::{IntoResponse, Response},
  routing::get,
  serve as axum_serve, Json, Router,
};
use clap::Parser;
use mina_ocv::{shutdown_signal, Ocv, OcvConfig, Wrapper};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Clone, Parser)]
struct ServeArgs {
  /// API Host.
  #[clap(long, env, default_value = "127.0.0.1")]
  pub host: String,
  /// API Port.
  #[clap(long, env, default_value = "8080")]
  pub port: u16,
  /// OCV Args.
  #[command(flatten)]
  pub config: OcvConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
  let ServeArgs { host, port, config } = ServeArgs::parse();
  tracing_subscriber::fmt::init();

  let listener = TcpListener::bind(format!("{}:{}", host, port)).await?;
  tracing::info!("Starting server at http://{}.", listener.local_addr()?);

  let ocv = config.to_ocv().await?;
  let router = Router::new()
    .route("/api/info", get(get_info))
    .route("/api/proposals", get(get_proposals))
    .route("/api/proposal/:id", get(get_proposal))
    .route("/api/proposal/:id/results", get(get_proposal_result))
    .layer(CorsLayer::permissive())
    .with_state(Arc::new(ocv));
  axum_serve(listener, router).with_graceful_shutdown(shutdown_signal()).await?;
  Ok(())
}

#[debug_handler]
async fn get_info(ctx: State<Arc<Ocv>>) -> Response {
  tracing::info!("get_info");
  Wrapper(ctx.info().await).into_response()
}

#[debug_handler]
async fn get_proposals(ctx: State<Arc<Ocv>>) -> Response {
  tracing::info!("get_proposals");
  Json(ctx.proposals_manifest.proposals.clone()).into_response()
}

#[debug_handler]
async fn get_proposal(ctx: State<Arc<Ocv>>, Path(id): Path<usize>) -> Response {
  tracing::info!("get_proposal {}", id);
  Wrapper(ctx.proposal(id).await).into_response()
}

#[debug_handler]
async fn get_proposal_result(ctx: State<Arc<Ocv>>, Path(id): Path<usize>) -> Response {
  tracing::info!("get_proposal_result {}", id);
  Wrapper(ctx.proposal_result(id).await).into_response()
}
