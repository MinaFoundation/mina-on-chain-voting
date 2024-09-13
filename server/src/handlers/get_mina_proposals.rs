use crate::{prelude::*, Context};
use axum::{response::IntoResponse, Extension, Json};
use reqwest::StatusCode;

pub async fn get_mina_proposals(ctx: Extension<Context>) -> Result<impl IntoResponse> {
  Ok((StatusCode::OK, Json(ctx.manifest.proposals.clone())).into_response())
}
