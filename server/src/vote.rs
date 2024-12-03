use crate::{archive::FetchTransactionResult, ledger::Ledger, Proposal, Wrapper};
use anyhow::{Context, Result};
use diesel::SqlType;
use diesel_derive_enum::DbEnum;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap};

#[derive(SqlType)]
#[diesel(postgres_type(name = "chain_status_type"))]
pub struct ChainStatusType;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, DbEnum)]
#[ExistingTypePath = "ChainStatusType"]
pub enum BlockStatus {
  Pending,
  Canonical,
  Orphaned,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct VoteWithWeight {
  pub account: String,
  pub hash: String,
  pub memo: String,
  pub height: i64,
  pub status: BlockStatus,
  pub timestamp: i64,
  pub nonce: i64,
  pub weight: Decimal,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Vote {
  pub account: String,
  pub hash: String,
  pub memo: String,
  pub height: i64,
  pub status: BlockStatus,
  pub timestamp: i64,
  pub nonce: i64,
}

impl Vote {
  pub fn new(
    account: impl Into<String>,
    hash: impl Into<String>,
    memo: impl Into<String>,
    height: i64,
    status: BlockStatus,
    timestamp: i64,
    nonce: i64,
  ) -> Self {
    Self { account: account.into(), hash: hash.into(), memo: memo.into(), height, status, timestamp, nonce }
  }

  pub fn to_weighted(&self, weight: Decimal) -> VoteWithWeight {
    VoteWithWeight {
      account: self.account.clone(),
      hash: self.hash.clone(),
      memo: self.memo.clone(),
      height: self.height,
      status: self.status,
      timestamp: self.timestamp,
      nonce: self.nonce,
      weight,
    }
  }

  pub fn update_memo(&mut self, memo: impl Into<String>) {
    let memo = memo.into();
    self.memo = memo;
  }

  pub fn update_status(&mut self, status: BlockStatus) {
    self.status = status;
  }

  pub fn is_newer_than(&self, other: &Vote) -> bool {
    self.height > other.height || (self.height == other.height && self.nonce > other.nonce)
  }

  pub fn match_decoded_memo(&mut self, key: &str) -> Option<String> {
    if let Ok(decoded) = self.decode_memo() {
      if decoded.to_lowercase() == key.to_lowercase() || decoded.to_lowercase() == format!("no {}", key.to_lowercase())
      {
        return Some(decoded);
      }
    }
    None
  }

  pub fn match_decoded_mef_memo(&mut self, key: &str) -> Option<String> {
    if let Ok(decoded) = self.decode_memo() {
      if decoded.to_lowercase() == format!("yes id {}", key) || decoded.to_lowercase() == format!("no id {}", key)
      {
        return Some(decoded);
      }
    }
    None
  }

  pub(crate) fn decode_memo(&self) -> Result<String> {
    let decoded =
      bs58::decode(&self.memo).into_vec().with_context(|| format!("failed to decode memo {} - bs58", &self.memo))?;

    let value = &decoded[3 .. decoded[2] as usize + 3];

    let result =
      String::from_utf8(value.to_vec()).with_context(|| format!("failed to decode memo {} - from_utf8", &self.memo))?;
    Ok(result)
  }
}

impl From<FetchTransactionResult> for Vote {
  fn from(res: FetchTransactionResult) -> Self {
    Vote::new(res.account, res.hash, res.memo, res.height, res.status, res.timestamp, res.nonce)
  }
}

impl Wrapper<Vec<Vote>> {
  pub fn process(self, key: impl Into<String>, tip: i64) -> Wrapper<HashMap<String, Vote>> {
    let mut map = HashMap::new();
    let key = key.into();

    for mut vote in self.0 {
      if let Some(memo) = vote.match_decoded_memo(&key) {
        vote.update_memo(memo);

        if tip - vote.height >= 10 {
          vote.update_status(BlockStatus::Canonical);
        }

        match map.entry(vote.account.clone()) {
          Entry::Vacant(e) => {
            e.insert(vote);
          }
          Entry::Occupied(mut e) => {
            let current_vote = e.get_mut();
            if vote.is_newer_than(current_vote) {
              *current_vote = vote;
            }
          }
        }
      }
    }

    Wrapper(map)
  }

  pub fn process_mep(self, id: usize, tip: i64) -> Wrapper<HashMap<String, Vote>> {
    let mut map = HashMap::new();
    let id_str = id.to_string();

    for mut vote in self.0 {
      if let Some(memo) = vote.match_decoded_mef_memo(&id_str) {
        vote.update_memo(memo);

        if tip - vote.height >= 10 {
          vote.update_status(BlockStatus::Canonical);
        }

        match map.entry(vote.account.clone()) {
          Entry::Vacant(e) => {
            e.insert(vote);
          }
          Entry::Occupied(mut e) => {
            let current_vote = e.get_mut();
            if vote.is_newer_than(current_vote) {
              *current_vote = vote;
            }
          }
        }
      }
    }

    Wrapper(map)
  }

  pub fn into_weighted(self, proposal: &Proposal, ledger: &Ledger, tip: i64) -> Wrapper<Vec<VoteWithWeight>> {
    let votes = self.process(&proposal.key, tip);

    let votes_with_stake: Vec<VoteWithWeight> = votes
      .0
      .iter()
      .filter_map(|(account, vote)| {
        let stake = ledger.get_stake_weight(&votes, &proposal.version, account).ok()?;
        Some(vote.to_weighted(stake))
      })
      .collect();

    Wrapper(votes_with_stake)
  }

  pub fn into_weighted_mep(self, id: usize, ledger: &Ledger, tip: i64) -> Wrapper<Vec<VoteWithWeight>> {
    let votes = self.process_mep(id, tip);

    let votes_with_stake: Vec<VoteWithWeight> = votes
      .0
      .iter()
      .filter_map(|(account, vote)| {
        let stake = ledger.get_stake_weight_mep(&votes, account).ok()?;
        Some(vote.to_weighted(stake))
      })
      .collect();

    Wrapper(votes_with_stake)
  }
}

impl Wrapper<HashMap<String, Vote>> {
  pub fn to_vec(&self) -> Wrapper<Vec<Vote>> {
    Wrapper(self.0.values().cloned().collect())
  }
}

impl Wrapper<HashMap<String, Vote>> {
  pub fn sort_by_timestamp(&mut self) -> &Self {
    self.to_vec().0.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    self
  }
}
impl Wrapper<Vec<VoteWithWeight>> {
  pub fn sort_by_timestamp(mut self) -> Self {
    self.0.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_decode_memo() {
    let mut vote = Vote::new("1", "1", "", 100, BlockStatus::Pending, 100, 1);

    vote.update_memo("E4Yf92G48v8FApR4EWQq3iKb2vZkHHxZHPaZ73NQNBXmHeXNzHHSp");
    assert_eq!(vote.decode_memo().unwrap(), "Payment#0");

    vote.update_memo("E4YjFkHVUXbEAkQcUrAEcS1fqvbncnn9Tuz2Jtb1Uu79zY9UAJRpd");
    assert_eq!(vote.decode_memo().unwrap(), "no cftest-2");

    vote.update_memo("E4ZJ3rmurwsMFrSvLdSAGRqmXRjYeZt84Wws4dixfpN67Xj7SrRLu");
    assert_eq!(vote.decode_memo().unwrap(), "MinaExplorer Gas Fee Service");

    vote.update_memo("E4YM2vTHhWEg66xpj52JErHUBU4pZ1yageL4TVDDpTTSsv8mK6YaH");
    assert_eq!(vote.decode_memo().unwrap(), "");
  }

  #[test]
  fn test_match_decode_memo() {
    let key = "cftest-2";
    let mut votes = get_test_votes();

    let v1_decoded = votes[0].match_decoded_memo(key).unwrap();
    let v2_decoded = votes[1].match_decoded_memo(key).unwrap();
    let v3_decoded = votes[2].match_decoded_memo(key).unwrap();
    let v4_decoded = votes[3].match_decoded_memo(key).unwrap();
    let v5_decoded = votes[4].match_decoded_memo(key);

    assert_eq!(v1_decoded, "no cftest-2");
    assert_eq!(v2_decoded, "no cftest-2");
    assert_eq!(v3_decoded, "no cftest-2");
    assert_eq!(v4_decoded, "cftest-2");
    assert_eq!(v5_decoded, None);
  }

  #[test]
  fn test_process_votes() {
    let votes = get_test_votes();
    let binding = Wrapper(votes).process("cftest-2", 129);
    let processed = binding.to_vec().0;

    assert_eq!(processed.len(), 2);

    let a1 = processed.iter().find(|s| s.account == "1").unwrap();
    let a2 = processed.iter().find(|s| s.account == "2").unwrap();

    assert_eq!(a1.account, "1");
    assert_eq!(a1.hash, "2");
    assert_eq!(a1.memo, "no cftest-2");
    assert_eq!(a1.height, 110);
    assert_eq!(a1.status, BlockStatus::Canonical);
    assert_eq!(a1.nonce, 1);

    assert_eq!(a2.account, "2");
    assert_eq!(a2.hash, "4");
    assert_eq!(a2.memo, "cftest-2");
    assert_eq!(a2.height, 120);
    assert_eq!(a2.status, BlockStatus::Pending);
    assert_eq!(a2.nonce, 2);
  }

  fn get_test_votes() -> Vec<Vote> {
    vec![
      Vote::new("1", "1", "E4YjFkHVUXbEAkQcUrAEcS1fqvbncnn9Tuz2Jtb1Uu79zY9UAJRpd", 100, BlockStatus::Pending, 100, 1),
      Vote::new("1", "2", "E4YjFkHVUXbEAkQcUrAEcS1fqvbncnn9Tuz2Jtb1Uu79zY9UAJRpd", 110, BlockStatus::Pending, 110, 1),
      Vote::new("2", "3", "E4YjFkHVUXbEAkQcUrAEcS1fqvbncnn9Tuz2Jtb1Uu79zY9UAJRpd", 110, BlockStatus::Pending, 110, 1),
      Vote::new("2", "4", "E4YdLeukpqzqyBAxujeELx9SZWoUW9MhcUfnGHF9PhQmxTJcpmj7j", 120, BlockStatus::Pending, 120, 2),
      Vote::new("2", "4", "E4YiC7vB4DC9JoQvaj83nBWwHC3gJh4G9EBef7xh4ti4idBAgZai7", 120, BlockStatus::Pending, 120, 2),
    ]
  }
}
