use crate::{
  archive::{fetch_chain_tip, fetch_transactions},
  MinaProposal, MinaVote, Ocv, Wrapper,
};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct GetMinaProposalResponse {
  #[serde(flatten)]
  proposal: MinaProposal,
  votes: Vec<MinaVote>,
}

impl Ocv {
  pub async fn get_mina_proposal(&self, id: usize) -> Result<GetMinaProposalResponse> {
    let proposal = self.manifest.proposal(id)?;

    if let Some(cached) = self.cache.votes.get(&proposal.key).await {
      return Ok(GetMinaProposalResponse { proposal, votes: cached.to_vec() });
    }

    let transactions = fetch_transactions(&self.conn_manager, proposal.start_time, proposal.end_time)?;

    let chain_tip = fetch_chain_tip(&self.conn_manager)?;

    let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
      .process(&proposal.key, chain_tip)
      .sort_by_timestamp()
      .to_vec()
      .0;

    self.cache.votes.insert(proposal.key.clone(), Arc::new(votes.clone())).await;

    Ok(GetMinaProposalResponse { proposal, votes })
  }
}
