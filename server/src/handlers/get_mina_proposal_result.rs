use crate::{
  archive::{fetch_chain_tip, fetch_transactions},
  prelude::*,
  Context, Ledger, MinaProposal, MinaVoteWithWeight,
};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
struct GetMinaProposalResultResponse {
  #[serde(flatten)]
  proposal: MinaProposal,
  total_stake_weight: Decimal,
  positive_stake_weight: Decimal,
  negative_stake_weight: Decimal,
  votes: Vec<MinaVoteWithWeight>,
}

pub async fn get_mina_proposal_result(ctx: Extension<Context>, Path(id): Path<usize>) -> Result<impl IntoResponse> {
  let proposal = ctx.manifest.proposal(id)?;
  if proposal.ledger_hash.is_none() {
    let response = GetMinaProposalResultResponse {
      proposal,
      total_stake_weight: Decimal::ZERO,
      positive_stake_weight: Decimal::ZERO,
      negative_stake_weight: Decimal::ZERO,
      votes: Vec::new(),
    };
    return Ok((StatusCode::OK, Json(response)).into_response());
  }
  let hash = proposal.ledger_hash.clone().expect("hash should always be present");

  let votes = if let Some(cached_votes) = ctx.cache.votes_weighted.get(&proposal.key).await {
    cached_votes.to_vec()
  } else {
    let transactions = fetch_transactions(&ctx.conn_manager, proposal.start_time, proposal.end_time)?;

    let chain_tip = fetch_chain_tip(&ctx.conn_manager)?;

    let ledger = if let Some(cached_ledger) = ctx.cache.ledger.get(&hash).await {
      Ledger(cached_ledger.to_vec())
    } else {
      let ledger =
        Ledger::fetch(&hash, ctx.ledger_storage_path.clone(), ctx.network, ctx.bucket_name.clone(), proposal.epoch)
          .await?;

      ctx.cache.ledger.insert(hash, Arc::new(ledger.0.clone())).await;

      ledger
    };

    let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
      .into_weighted(&proposal, &ledger, chain_tip)
      .sort_by_timestamp()
      .0;

    ctx.cache.votes_weighted.insert(proposal.key.clone(), Arc::new(votes.clone())).await;

    votes
  };

  let mut positive_stake_weight = Decimal::from(0);
  let mut negative_stake_weight = Decimal::from(0);

  for vote in &votes {
    if vote.memo.split_whitespace().next().eq(&Some("no")) {
      negative_stake_weight += vote.weight;
    } else {
      positive_stake_weight += vote.weight;
    }
  }

  let response = GetMinaProposalResultResponse {
    proposal,
    total_stake_weight: positive_stake_weight + negative_stake_weight,
    positive_stake_weight,
    negative_stake_weight,
    votes,
  };

  Ok((StatusCode::OK, Json(response)).into_response())
}
