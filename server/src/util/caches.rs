use crate::{ledger::LedgerAccount, Vote, VoteWithWeight};
use moka::future::Cache as MokaCache;
use std::sync::Arc;

#[derive(Clone)]
pub struct Caches {
  pub votes: MokaCache<String, Arc<Vec<Vote>>>,
  pub votes_weighted: MokaCache<String, Arc<Vec<VoteWithWeight>>>,
  pub ledger: MokaCache<String, Arc<Vec<LedgerAccount>>>,
}

impl Caches {
  pub fn build() -> Self {
    Self {
      votes: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      votes_weighted: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      ledger: MokaCache::builder().time_to_live(std::time::Duration::from_secs(60 * 60 * 12)).build(),
    }
  }
}
