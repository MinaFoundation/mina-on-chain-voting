use crate::{ledger::LedgerAccount, MinaVote, MinaVoteWithWeight};
use moka::future::Cache as MokaCache;
use std::sync::Arc;

type ArcVotes = Arc<Vec<MinaVote>>;
type ArcVotesWeighted = Arc<Vec<MinaVoteWithWeight>>;
type ArcLedger = Arc<Vec<LedgerAccount>>;

pub type VotesCache = MokaCache<String, ArcVotes>;
pub type VotesWeightedCache = MokaCache<String, ArcVotesWeighted>;
pub type LedgerCache = MokaCache<String, ArcLedger>;

pub struct CacheManager {
  pub votes: VotesCache,
  pub votes_weighted: VotesWeightedCache,
  pub ledger: LedgerCache,
}

impl CacheManager {
  pub fn build() -> CacheManager {
    CacheManager {
      votes: VotesCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      votes_weighted: VotesWeightedCache::builder().time_to_live(std::time::Duration::from_secs(60 * 5)).build(),
      ledger: LedgerCache::builder().time_to_live(std::time::Duration::from_secs(60 * 60 * 12)).build(),
    }
  }
}
