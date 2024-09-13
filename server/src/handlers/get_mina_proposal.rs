use crate::{
  archive::{fetch_chain_tip, fetch_transactions},
  prelude::*,
  Context, MinaProposal, MinaVote,
};
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
struct GetMinaProposalResponse {
  #[serde(flatten)]
  proposal: MinaProposal,
  votes: Vec<MinaVote>,
}

pub async fn get_mina_proposal(ctx: Extension<Context>, Path(id): Path<usize>) -> Result<impl IntoResponse> {
  let proposal = ctx.manifest.proposal(id)?;

  if let Some(cached) = ctx.cache.votes.get(&proposal.key).await {
    let response = GetMinaProposalResponse { proposal, votes: cached.to_vec() };

    return Ok((StatusCode::OK, Json(response)).into_response());
  }

  let transactions = fetch_transactions(&ctx.conn_manager, proposal.start_time, proposal.end_time)?;

  let chain_tip = fetch_chain_tip(&ctx.conn_manager)?;

  let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
    .process(&proposal.key, chain_tip)
    .sort_by_timestamp()
    .to_vec()
    .0;

  ctx.cache.votes.insert(proposal.key.clone(), Arc::new(votes.clone())).await;

  let response = GetMinaProposalResponse { proposal, votes };

  Ok((StatusCode::OK, Json(response)).into_response())
}
