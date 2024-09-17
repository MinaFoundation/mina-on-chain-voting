use anyhow::Result;
use axum::{
  extract::{Path, State},
  response::{IntoResponse, Response},
  routing::get,
  serve as axum_serve, Json, Router,
};
use clap::Parser;
use mina_ocv::{Ocv, OcvConfig, Wrapper};
use std::sync::Arc;
use tokio::net::TcpListener;

// use tower_http::cors::{Any, CorsLayer};

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
    .route("/api/info", get(get_core_api_info))
    .route("/api/proposals", get(get_mina_proposals))
    .route("/api/proposal/:id", get(get_mina_proposal))
    .route("/api/proposal/:id/results", get(get_mina_proposal_result))
    .with_state(Arc::new(ocv));
  axum_serve(listener, router).await?;
  Ok(())
}

async fn get_core_api_info(ctx: State<Arc<Ocv>>) -> Response {
  Wrapper(ctx.info().await).wrapper_into_response()
}

async fn get_mina_proposals(ctx: State<Arc<Ocv>>) -> Response {
  Json(ctx.manifest.proposals.clone()).into_response()
}

async fn get_mina_proposal_result(ctx: State<Arc<Ocv>>, Path(id): Path<usize>) -> Response {
  Wrapper(ctx.get_mina_proposal_result(id).await).wrapper_into_response()
}

async fn get_mina_proposal(ctx: State<Arc<Ocv>>, Path(id): Path<usize>) -> Response {
  Wrapper(ctx.get_mina_proposal(id).await).wrapper_into_response()
}
