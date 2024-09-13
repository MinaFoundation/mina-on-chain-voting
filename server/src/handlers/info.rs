use crate::{
  archive::{fetch_chain_tip, fetch_latest_slot},
  prelude::*,
  Context,
};
use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct GetCoreApiInfoResponse {
  chain_tip: i64,
  current_slot: i64,
}

#[allow(clippy::unused_async)]
pub async fn get_core_api_info(ctx: Extension<Context>) -> Result<impl IntoResponse> {
  let chain_tip = fetch_chain_tip(&ctx.conn_manager)?;
  let current_slot = fetch_latest_slot(&ctx.conn_manager)?;
  let response = GetCoreApiInfoResponse { chain_tip, current_slot };
  Ok((StatusCode::OK, Json(response)).into_response())
}
