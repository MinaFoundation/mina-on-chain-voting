use crate::{util::Caches, Archive, Ledger, Network, Proposal, Vote, VoteWithWeight, Wrapper};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct Ocv {
  pub caches: Caches,
  pub archive: Archive,
  pub network: Network,
  pub ledger_storage_path: PathBuf,
  pub bucket_name: String,
  pub proposals: Vec<Proposal>,
}

impl Ocv {
  pub async fn info(&self) -> Result<GetCoreApiInfoResponse> {
    let chain_tip = self.archive.fetch_chain_tip()?;
    let current_slot = self.archive.fetch_latest_slot()?;
    Ok(GetCoreApiInfoResponse { chain_tip, current_slot })
  }

  pub async fn proposal(&self, id: usize) -> Result<ProposalResponse> {
    let proposal = self.find_proposal(id)?;

    if let Some(cached) = self.caches.votes.get(&proposal.key).await {
      return Ok(ProposalResponse { proposal, votes: cached.to_vec() });
    }

    let transactions = self.archive.fetch_transactions(proposal.start_time, proposal.end_time)?;

    let chain_tip = self.archive.fetch_chain_tip()?;

    let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
      .process(&proposal.key, chain_tip)
      .sort_by_timestamp()
      .to_vec()
      .0;

    self.caches.votes.insert(proposal.key.clone(), Arc::new(votes.clone())).await;

    Ok(ProposalResponse { proposal, votes })
  }

  pub async fn proposal_consideration(&self, id: usize) -> Result<GetMinaProposalConsiderationResponse> {
    // Fetch the proposal
    let proposal = self.find_proposal(id)?;
    let hash = match proposal.ledger_hash.clone() {
        None => {
            return Ok(GetMinaProposalConsiderationResponse {
                proposal,
                total_community_votes: Decimal::ZERO,
                total_reviewer_votes: Decimal::ZERO,
                total_stake_weight: Decimal::ZERO,
                positive_stake_weight: Decimal::ZERO,
                negative_stake_weight: Decimal::ZERO,
                votes: Vec::new(),
                vote_status: "Proposal not initialized".to_string(),
            });
        }
        Some(value) => value,
    };

    // // Step 1: retrieve votes
    // let cached_votes = self.caches.votes_weighted.get(&proposal.key).await;
    // let votes = cached_votes.unwrap_or_default().to_vec();

    // // Step 2: check eligibility
    // let transactions = self.archive.fetch_transactions(proposal.start_time, proposal.end_time)?;
    // let chain_tip = self.archive.fetch_chain_tip()?;
    // let ledger = if let Some(cached_ledger) = self.caches.ledger.get(&hash).await {
    //     Ledger(cached_ledger.to_vec())
    // } else {
    //     let ledger = Ledger::fetch(self, &hash).await?;
    //     self.caches.ledger.insert(hash, Arc::new(ledger.0.clone())).await;
    //     ledger
    // };
    let votes = if let Some(cached_votes) = self.caches.votes_weighted.get(&proposal.key).await {
      cached_votes.to_vec()
    } else {
      let transactions = self.archive.fetch_transactions(proposal.start_time, proposal.end_time)?;

      let chain_tip = self.archive.fetch_chain_tip()?;

      let ledger = if let Some(cached_ledger) = self.caches.ledger.get(&hash).await {
        Ledger(cached_ledger.to_vec())
      } else {
        let ledger = Ledger::fetch(self, &hash).await?;

        self.caches.ledger.insert(hash, Arc::new(ledger.0.clone())).await;

        ledger
      };

      let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
        .into_weighted(&proposal, &ledger, chain_tip)
        .sort_by_timestamp()
        .0;

      self.caches.votes_weighted.insert(proposal.key.clone(), Arc::new(votes.clone())).await;

      votes
    };


    // Calculate votes count and weighted votes
    let mut positive_stake_weight = Decimal::from(0);
    let mut negative_stake_weight = Decimal::from(0);
    let mut total_community_votes = Decimal::from(0);
    let mut total_reviewer_votes = Decimal::from(0);

    // let votes = transactions
    //     .into_iter()
    //     .filter(|tx| eligible_voters.contains(&tx.sender))
    //     .map(|tx| Vote {
    //         voter: tx.sender.clone(),
    //         weight: ledger.get(&tx.sender).unwrap().balance,
    //         memo: tx.memo.clone(),
    //     })
    //     .collect::<Vec<Vote>>();

    for vote in &votes {
        if vote.memo.split_whitespace().next().eq(&Some("NOID")) {
            negative_stake_weight += vote.weight;
        }
        if vote.memo.split_whitespace().next().eq(&Some("YESID")) {
            positive_stake_weight += vote.weight;
        }
        positive_stake_weight += vote.weight;
        total_community_votes += Decimal::from(1);
    }

    // Check if enough votes
    if total_community_votes < Decimal::from(10) {
      return Ok(GetMinaProposalConsiderationResponse {
        proposal,
        total_community_votes: Decimal::ZERO,
        total_reviewer_votes: Decimal::ZERO,
        total_stake_weight: Decimal::ZERO,
        positive_stake_weight: Decimal::ZERO,
        negative_stake_weight: Decimal::ZERO,
        votes: Vec::new(),
        vote_status: "Insufficient voters".to_string(),
    });
    }

    // Step 4: voting results
    let total_stake_weight = positive_stake_weight + negative_stake_weight;
    if votes.len() < 10 {
        return Ok(GetMinaProposalConsiderationResponse {
            proposal,
            total_community_votes,
            total_reviewer_votes,
            total_stake_weight,
            positive_stake_weight,
            negative_stake_weight,
            votes,
            vote_status: "Insufficient community votes".to_string(),
        });
    }

    Ok(GetMinaProposalConsiderationResponse {
        proposal,
        total_community_votes,
        total_reviewer_votes,
        total_stake_weight,
        positive_stake_weight,
        negative_stake_weight,
        votes,
        vote_status: "Proposal selected for the next phase".to_string(),
    })
}


  pub async fn proposal_result(&self, id: usize) -> Result<GetMinaProposalResultResponse> {
    let proposal = self.find_proposal(id)?;
    let hash = match proposal.ledger_hash.clone() {
      None => {
        return Ok(GetMinaProposalResultResponse {
          proposal,
          total_stake_weight: Decimal::ZERO,
          positive_stake_weight: Decimal::ZERO,
          negative_stake_weight: Decimal::ZERO,
          votes: Vec::new(),
        });
      }
      Some(value) => value,
    };

    let votes = if let Some(cached_votes) = self.caches.votes_weighted.get(&proposal.key).await {
      cached_votes.to_vec()
    } else {
      let transactions = self.archive.fetch_transactions(proposal.start_time, proposal.end_time)?;

      let chain_tip = self.archive.fetch_chain_tip()?;

      let ledger = if let Some(cached_ledger) = self.caches.ledger.get(&hash).await {
        Ledger(cached_ledger.to_vec())
      } else {
        let ledger = Ledger::fetch(self, &hash).await?;

        self.caches.ledger.insert(hash, Arc::new(ledger.0.clone())).await;

        ledger
      };

      let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
        .into_weighted(&proposal, &ledger, chain_tip)
        .sort_by_timestamp()
        .0;

      self.caches.votes_weighted.insert(proposal.key.clone(), Arc::new(votes.clone())).await;

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

    Ok(GetMinaProposalResultResponse {
      proposal,
      total_stake_weight: positive_stake_weight + negative_stake_weight,
      positive_stake_weight,
      negative_stake_weight,
      votes,
    })
  }

  fn find_proposal(&self, id: usize) -> Result<Proposal> {
    Ok(self.proposals.iter().find(|proposal| proposal.id == id).ok_or(anyhow!("Proposal {id} dne."))?.to_owned())
  }
}

#[derive(Serialize)]
pub struct GetCoreApiInfoResponse {
  chain_tip: i64,
  current_slot: i64,
}

#[derive(Serialize)]
pub struct ProposalResponse {
  #[serde(flatten)]
  proposal: Proposal,
  votes: Vec<Vote>,
}

#[derive(Serialize)]
pub struct GetMinaProposalResultResponse {
  #[serde(flatten)]
  proposal: Proposal,
  total_stake_weight: Decimal,
  positive_stake_weight: Decimal,
  negative_stake_weight: Decimal,
  votes: Vec<VoteWithWeight>,
}

#[derive(Serialize)]
pub struct GetMinaProposalConsiderationResponse {
  #[serde(flatten)]
  proposal: Proposal,
  total_community_votes: Decimal,
  total_reviewer_votes: Decimal,
  total_stake_weight: Decimal,
  positive_stake_weight: Decimal,
  negative_stake_weight: Decimal,
  votes: Vec<VoteWithWeight>,
  vote_status: String,
   // eligible_voters: Vec<String>,
  // positive_reviewers_count: i64,
  // positive_community_count: i64,
  // votes: Vec<Vote>,
}