use std::sync::Arc;

use anyhow::Result;
use axum::{
  Json, Router, debug_handler,
  extract::{Path, State},
  response::IntoResponse,
  routing::get,
  serve as axum_serve,
};
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::{Ocv, OcvConfig, Wrapper, shutdown_signal};

#[derive(Clone, Parser)]
pub struct ServeArgs {
  /// API Host.
  #[clap(long, env, default_value = "0.0.0.0")]
  pub host: String,
  /// API Port.
  #[clap(long, env, default_value = "8080")]
  pub port: u16,
  /// OCV Args.
  #[command(flatten)]
  pub config: OcvConfig,
}

impl ServeArgs {
  pub async fn serve(&self) -> Result<()> {
    tracing_subscriber::fmt::init();

    let listener = TcpListener::bind(format!("{}:{}", self.host, self.port)).await?;
    tracing::info!("Starting server at http://{}.", listener.local_addr()?);

    let ocv = self.config.to_ocv().await?;
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
}

#[debug_handler]
async fn get_info(ctx: State<Arc<Ocv>>) -> impl IntoResponse {
  tracing::info!("get_info");
  Wrapper(ctx.info().await)
}

#[debug_handler]
async fn get_proposals(ctx: State<Arc<Ocv>>) -> impl IntoResponse {
  tracing::info!("get_proposals");
  Json(ctx.proposals.to_owned())
}

#[debug_handler]
async fn get_proposal(ctx: State<Arc<Ocv>>, Path(id): Path<usize>) -> impl IntoResponse {
  tracing::info!("get_proposal {}", id);
  Wrapper(ctx.proposal(id).await)
}

#[debug_handler]
async fn get_proposal_result(ctx: State<Arc<Ocv>>, Path(id): Path<usize>) -> impl IntoResponse {
  tracing::info!("get_proposal_result {}", id);
  Wrapper(ctx.proposal_result(id).await)
}
