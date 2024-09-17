use crate::{
  archive::{fetch_chain_tip, fetch_latest_slot},
  Ocv,
};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct GetCoreApiInfoResponse {
  chain_tip: i64,
  current_slot: i64,
}

impl Ocv {
  pub async fn info(&self) -> Result<GetCoreApiInfoResponse> {
    let chain_tip = fetch_chain_tip(&self.conn_manager)?;
    let current_slot = fetch_latest_slot(&self.conn_manager)?;
    Ok(GetCoreApiInfoResponse { chain_tip, current_slot })
  }
}
