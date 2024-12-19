use std::{path::PathBuf, sync::Arc};

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{Archive, Ledger, Network, Proposal, ReleaseStage, Vote, VoteWithWeight, Wrapper, util::Caches};

#[derive(Clone)]
pub struct Ocv {
  pub caches: Caches,
  pub archive: Archive,
  pub network: Network,
  pub release_stage: ReleaseStage,
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

  /// Checks whether the positive community vote threshold has been met based
  /// on the release stage.
  ///
  /// # Arguments
  ///
  /// * `total_positive_community_votes` - Total number of positive votes from
  ///   the community.
  /// * `total_negative_community_votes` - Total number of negative votes from
  ///   the community.
  ///
  /// # Returns
  ///
  /// Returns `true` if the positive votes meet the threshold for the current
  /// release stage, otherwise `false`.
  ///
  /// # Description
  ///
  /// - For the `Production` release stage, the minimum threshold for positive
  ///   votes is 10.
  /// - For other release stages, the threshold is 2.
  pub fn has_met_vote_threshold(
    &self,
    total_positive_community_votes: usize,
    _total_negative_community_votes: usize,
  ) -> bool {
    let min_positive_votes = if self.release_stage == ReleaseStage::Production { 10 } else { 2 };
    tracing::info!("min_positive_votes {}", min_positive_votes);
    tracing::info!("release_stage {}", self.release_stage);
    total_positive_community_votes >= min_positive_votes
  }

  pub async fn proposal_consideration(
    &self,
    id: usize,
    start_time: i64,
    end_time: i64,
    ledger_hash: Option<String>,
  ) -> Result<GetMinaProposalConsiderationResponse> {
    let proposal_key = "MEF".to_string() + &id.to_string();
    let votes = if let Some(cached_votes) = self.caches.votes.get(&proposal_key).await {
      cached_votes.to_vec()
    } else {
      let transactions = self.archive.fetch_transactions(start_time, end_time)?;

      let chain_tip = self.archive.fetch_chain_tip()?;
      let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
        .process_mep(id, chain_tip)
        .sort_by_timestamp()
        .to_vec()
        .0;

      self.caches.votes.insert(proposal_key.clone(), Arc::new(votes.clone())).await;
      tracing::info!("votes {}", votes.len());
      votes
    };
    // weighted votes
    let mut positive_stake_weight = Decimal::from(0);
    let mut negative_stake_weight = Decimal::from(0);

    // check community votes
    let mut total_positive_community_votes = 0;
    let mut total_negative_community_votes = 0;
    for vote in &votes {
      if vote.memo.to_lowercase() == format!("yes {}", id) {
        total_positive_community_votes += 1;
      }
      if vote.memo.to_lowercase() == format!("no {}", id) {
        total_negative_community_votes += 1;
      }
    }
    // Check if enough positive votes
    if !self.has_met_vote_threshold(total_positive_community_votes, total_negative_community_votes) {
      return Ok(GetMinaProposalConsiderationResponse {
        proposal_id: id,
        total_community_votes: votes.len(),
        total_positive_community_votes,
        total_negative_community_votes,
        total_stake_weight: Decimal::ZERO,
        positive_stake_weight: Decimal::ZERO,
        negative_stake_weight: Decimal::ZERO,
        elegible: false,
        vote_status: "Insufficient voters".to_string(),
        votes,
      });
    }

    // Calculate weighted votes if ledger_hash params is provided
    if let Some(hash) = ledger_hash {
      let votes_weighted = if let Some(cached_votes) = self.caches.votes_weighted.get(&proposal_key).await {
        cached_votes.to_vec()
      } else {
        let transactions = self.archive.fetch_transactions(start_time, end_time)?;

        let chain_tip = self.archive.fetch_chain_tip()?;

        let ledger = if let Some(cached_ledger) = self.caches.ledger.get(&hash).await {
          Ledger(cached_ledger.to_vec())
        } else {
          let ledger = Ledger::fetch(self, &hash).await?;

          self.caches.ledger.insert(hash, Arc::new(ledger.0.clone())).await;

          ledger
        };

        let votes = Wrapper(transactions.into_iter().map(std::convert::Into::into).collect())
          .into_weighted_mep(id, &ledger, chain_tip)
          .sort_by_timestamp()
          .0;

        self.caches.votes_weighted.insert(proposal_key.clone(), Arc::new(votes.clone())).await;

        votes
      };
      for vote in &votes_weighted {
        if vote.memo.to_lowercase() == format!("no {}", id) {
          negative_stake_weight += vote.weight;
        }
        if vote.memo.to_lowercase() == format!("yes {}", id) {
          positive_stake_weight += vote.weight;
        }
        positive_stake_weight += vote.weight;
      }
    } else {
      tracing::info!("ledger_hash is not provided.");
    }

    let total_stake_weight = positive_stake_weight + negative_stake_weight;

    // Voting results
    Ok(GetMinaProposalConsiderationResponse {
      proposal_id: id,
      total_community_votes: votes.len(),
      total_positive_community_votes,
      total_negative_community_votes,
      total_stake_weight,
      positive_stake_weight,
      negative_stake_weight,
      elegible: true,
      vote_status: "Proposal selected for the next phase".to_string(),
      votes,
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
  proposal_id: usize,
  total_community_votes: usize,
  total_positive_community_votes: usize,
  total_negative_community_votes: usize,
  total_stake_weight: Decimal,
  positive_stake_weight: Decimal,
  negative_stake_weight: Decimal,
  vote_status: String,
  elegible: bool,
  votes: Vec<Vote>,
}
