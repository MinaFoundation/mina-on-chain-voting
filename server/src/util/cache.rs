use crate::{ledger::LedgerAccount, Vote, VoteWithWeight};
use moka::future::Cache as MokaCache;
use std::sync::Arc;

type ArcVotes = Arc<Vec<Vote>>;
type ArcVotesWeighted = Arc<Vec<VoteWithWeight>>;
type ArcLedger = Arc<Vec<LedgerAccount>>;

type VotesCache = MokaCache<String, ArcVotes>;
type VotesWeightedCache = MokaCache<String, ArcVotesWeighted>;
type LedgerCache = MokaCache<String, ArcLedger>;

#[derive(Clone)]
pub struct Caches {
  pub votes: VotesCache,
  pub votes_weighted: VotesWeightedCache,
  pub ledger: LedgerCache,
}

impl Caches {
  pub fn build() -> Self {
    Self {
      votes: VotesCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      votes_weighted: VotesWeightedCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      ledger: LedgerCache::builder().time_to_live(std::time::Duration::from_secs(60 * 60 * 12)).build(),
    }
  }
}
