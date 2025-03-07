use std::sync::Arc;

use moka::future::Cache as MokaCache;

use crate::{RankedVote, Vote, VoteWithWeight, ledger::LedgerAccount};

#[derive(Clone)]
pub struct Caches {
  pub votes: MokaCache<String, Arc<Vec<Vote>>>,
  pub votes_weighted: MokaCache<String, Arc<Vec<VoteWithWeight>>>,
  pub ledger: MokaCache<String, Arc<Vec<LedgerAccount>>>,
  pub ranked_votes: MokaCache<String, Arc<Vec<RankedVote>>>,
}

impl Caches {
  pub fn build() -> Self {
    Self {
      votes: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      votes_weighted: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      ledger: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 60 * 12)).build(),
      ranked_votes: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
    }
  }
}
