use std::collections::{HashMap, hash_map::Entry};

use anyhow::{Context, Result};
use diesel::SqlType;
use diesel_derive_enum::DbEnum;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{Proposal, Wrapper, archive::FetchTransactionResult, ledger::Ledger};

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

  pub fn match_decoded_mef_memo(&mut self, round_id: &str, proposal_id: &str) -> Option<String> {
    if let Ok(decoded) = self.decode_memo() {
      if decoded.to_lowercase() == format!("mef{} yes {}", round_id, proposal_id)
        || decoded.to_lowercase() == format!("mef{} no {}", round_id, proposal_id)
      {
        return Some(decoded);
      }
    }
    None
  }

  pub fn parse_decoded_ranked_votes_memo(&mut self, key: &str) -> Option<(String, Vec<String>)> {
    if let Ok(decoded) = self.decode_memo() {
      let decoded = decoded.to_lowercase();
      tracing::info!("decoded memo: {}", decoded);
      // Split the decoded memo into prefix and proposals part
      if let Some((prefix, proposals_part)) = decoded.split_once(' ') {
        if prefix.starts_with("mef") {
          tracing::info!("mef vote: {}", prefix);
          let round_id = prefix.trim_start_matches("mef");
          tracing::info!("round id: {}", round_id);
          if round_id == key {
            // Split proposal IDs by whitespace
            let proposal_ids: Vec<String> = proposals_part.split_whitespace().map(|id| id.to_string()).collect();
            tracing::info!("proposals {}", proposals_part);
            return Some((round_id.to_string(), proposal_ids));
          }
        }
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

  pub fn process_mep(self, round_id: usize, proposal_id: usize, tip: i64) -> Wrapper<HashMap<String, Vote>> {
    let mut map = HashMap::new();
    let proposal_id_str = proposal_id.to_string();
    let round_id_str = round_id.to_string();

    for mut vote in self.0 {
      if let Some(memo) = vote.match_decoded_mef_memo(&round_id_str, &proposal_id_str) {
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

  pub fn into_weighted_mep(
    self,
    round_id: usize,
    proposal_id: usize,
    ledger: &Ledger,
    tip: i64,
  ) -> Wrapper<Vec<VoteWithWeight>> {
    let votes = self.process_mep(round_id, proposal_id, tip);

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

  #[test]
  fn test_decode_mep_memo() {
    let mut vote = Vote::new("1", "1", "", 100, BlockStatus::Pending, 100, 1);

    vote.update_memo("E4Yd67s51QN9DZVDy8JKPEoNGykMsYQ5KRiKpZHiLZTjA8dB9SnFT");
    assert_eq!(vote.decode_memo().unwrap(), "BeepBoop");

    vote.update_memo("E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K");
    assert_eq!(vote.decode_memo().unwrap(), "MEF1 YES 1");

    vote.update_memo("E4Yf7epFtpM8YAsxcGVagQQKmtUpwj8nKTWMQnWbXyhg7hE6ceJhJ");
    assert_eq!(vote.decode_memo().unwrap(), "MEF1 NO 1");
  }

  #[test]
  fn test_match_decode_mep_memo() {
    let round_id = "1";
    let proposal_id = "1";
    let mut votes = get_test_mep_votes();

    let v0_decoded = votes[0].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v1_decoded = votes[1].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v2_decoded = votes[2].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v3_decoded = votes[3].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v4_decoded = votes[4].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v5_decoded = votes[5].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v6_decoded = votes[6].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v7_decoded = votes[7].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v8_decoded = votes[8].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v9_decoded = votes[9].match_decoded_mef_memo(round_id, proposal_id).unwrap();
    let v10_decoded = votes[10].match_decoded_mef_memo(round_id, proposal_id).unwrap();

    assert_eq!(v0_decoded, "MEF1 YES 1");
    assert_eq!(v1_decoded, "MEF1 YES 1");
    assert_eq!(v2_decoded, "MEF1 YES 1");
    assert_eq!(v3_decoded, "MEF1 YES 1");
    assert_eq!(v4_decoded, "MEF1 YES 1");
    assert_eq!(v5_decoded, "MEF1 YES 1");
    assert_eq!(v6_decoded, "MEF1 YES 1");
    assert_eq!(v7_decoded, "MEF1 YES 1");
    assert_eq!(v8_decoded, "MEF1 YES 1");
    assert_eq!(v9_decoded, "MEF1 YES 1");
    assert_eq!(v10_decoded, "MEF1 NO 1");
  }

  #[test]
  fn test_process_mep_votes() {
    let votes = get_test_mep_votes();
    let binding = Wrapper(votes).process_mep(1, 1, 130);
    let processed = binding.to_vec().0;

    assert_eq!(processed.len(), 11);

    let a1 = processed.iter().find(|s| s.account == "1").unwrap();
    let a2 = processed.iter().find(|s| s.account == "2").unwrap();

    assert_eq!(a1.account, "1");
    assert_eq!(a1.hash, "1");
    assert_eq!(a1.memo, "MEF1 YES 1");
    assert_eq!(a1.height, 331718);
    assert_eq!(a1.status, BlockStatus::Canonical);
    assert_eq!(a1.nonce, 1);

    assert_eq!(a2.account, "2");
    assert_eq!(a2.hash, "2");
    assert_eq!(a2.memo, "MEF1 YES 1");
    assert_eq!(a2.height, 341719);
    assert_eq!(a2.status, BlockStatus::Pending);
    assert_eq!(a2.nonce, 2);
  }

  fn get_test_mep_votes() -> Vec<Vote> {
    vec![
      Vote::new(
        "1",
        "1",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        331718,
        BlockStatus::Canonical,
        1730897878000,
        1,
      ),
      Vote::new(
        "2",
        "2",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        341719,
        BlockStatus::Pending,
        1730897878000,
        2,
      ),
      Vote::new(
        "3",
        "3",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        351320,
        BlockStatus::Pending,
        1730897878000,
        3,
      ),
      Vote::new(
        "4",
        "4",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        352721,
        BlockStatus::Pending,
        1730897878000,
        4,
      ),
      Vote::new(
        "5",
        "5",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        353722,
        BlockStatus::Pending,
        1730897878000,
        5,
      ),
      Vote::new(
        "6",
        "6",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        354723,
        BlockStatus::Pending,
        1730897878000,
        6,
      ),
      Vote::new(
        "7",
        "7",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        355724,
        BlockStatus::Pending,
        1730897878000,
        7,
      ),
      Vote::new(
        "8",
        "8",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        356725,
        BlockStatus::Pending,
        1730897878000,
        8,
      ),
      Vote::new(
        "9",
        "9",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        357726,
        BlockStatus::Pending,
        1730897878000,
        9,
      ),
      Vote::new(
        "10",
        "10",
        "E4Yh4PzVLrCiugdoaASo5Ve6Do755ey6vGqkURC8z7qcADqMUcp9K",
        358727,
        BlockStatus::Pending,
        1730897878000,
        10,
      ),
      Vote::new(
        "11",
        "11",
        "E4Yf7epFtpM8YAsxcGVagQQKmtUpwj8nKTWMQnWbXyhg7hE6ceJhJ",
        358728,
        BlockStatus::Pending,
        1730897878000,
        11,
      ),
    ]
  }
}
