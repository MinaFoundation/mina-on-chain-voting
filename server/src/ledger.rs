use crate::{Network, ProposalVersion, Vote, Wrapper};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Read, path::Path};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ledger(pub Vec<LedgerAccount>);

impl Ledger {
  pub async fn fetch(
    hash: impl Into<String>,
    ledger_storage_path: String,
    network: Network,
    bucket_name: String,
    epoch: i64,
  ) -> Result<Ledger> {
    let hash: String = hash.into();
    let ledger_file_path = format!("{ledger_storage_path}/{network}-{epoch}-{hash}.json");
    if !Path::new(&ledger_file_path).exists() {
      if !Path::new(&ledger_storage_path).exists() {
        let _ = std::fs::create_dir_all(ledger_storage_path.clone());
      }
      let url =
        format!("https://{bucket_name}.s3.us-west-2.amazonaws.com/{network}/{network}-{epoch}-{hash}.json.tar.gz");
      tracing::info!("Ledger path not found, downloading {} to {}", url, ledger_file_path);
      let response = reqwest::get(url).await.unwrap();
      if response.status().is_success() {
        // Get the object body as bytes
        let body = response.bytes().await.unwrap();
        let tar_gz = flate2::read::GzDecoder::new(&body[..]);
        let mut archive = tar::Archive::new(tar_gz);
        archive.unpack(ledger_storage_path).unwrap();
      }
    }
    let mut bytes = Vec::new();
    println!("Trying to access: {}", ledger_file_path);
    std::fs::File::open(ledger_file_path).unwrap().read_to_end(&mut bytes).unwrap();
    Ok(Ledger(serde_json::from_slice(&bytes).unwrap()))
  }

  pub fn get_stake_weight(
    &self,
    map: &Wrapper<HashMap<String, Vote>>,
    version: &ProposalVersion,
    public_key: impl Into<String>,
  ) -> Result<Decimal> {
    let public_key: String = public_key.into();

    let account =
      self.0.iter().find(|d| d.pk == public_key).ok_or_else(|| anyhow!("account {public_key} not found in ledger"))?;

    let balance = account.balance.parse().unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE));

    match version {
      ProposalVersion::V1 => {
        if account.delegate.clone().unwrap_or(public_key.clone()) != public_key {
          return Ok(Decimal::new(0, LEDGER_BALANCE_SCALE));
        }

        let delegators = self
          .0
          .iter()
          .filter(|d| d.delegate.clone().unwrap_or(d.pk.clone()) == public_key && d.pk != public_key)
          .collect::<Vec<&LedgerAccount>>();

        if delegators.is_empty() {
          return Ok(balance);
        }

        let stake_weight = delegators.iter().fold(Decimal::new(0, LEDGER_BALANCE_SCALE), |acc, x| {
          x.balance.parse().unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE)) + acc
        });

        Ok(stake_weight + balance)
      }
      ProposalVersion::V2 => {
        let delegators = self
          .0
          .iter()
          .filter(|d| {
            d.delegate.clone().unwrap_or(d.pk.clone()) == public_key && d.pk != public_key && !map.0.contains_key(&d.pk)
          })
          .collect::<Vec<&LedgerAccount>>();

        if delegators.is_empty() {
          return Ok(balance);
        }

        let stake_weight = delegators.iter().fold(Decimal::new(0, LEDGER_BALANCE_SCALE), |acc, x| {
          x.balance.parse().unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE)) + acc
        });

        Ok(stake_weight + balance)
      }
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct LedgerAccount {
  pub pk: String,
  pub balance: String,
  pub delegate: Option<String>,
}

impl LedgerAccount {
  pub fn new(pk: String, balance: String, delegate: Option<String>) -> LedgerAccount {
    LedgerAccount { pk, balance, delegate }
  }
}

pub const LEDGER_BALANCE_SCALE: u32 = 9;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::BlockStatus;

  #[test]
  fn test_stake_weight_v1() {
    let (a, b, c, d, _) = get_accounts();
    let map = get_votes();

    // No account found - throw err.
    let error = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V1,
      "E",
    );
    assert!(error.is_err());

    // Delegated stake away - returns 0.000000000.
    let d_weight = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V1,
      "D",
    );
    assert_eq!(d_weight.unwrap(), Decimal::new(0, LEDGER_BALANCE_SCALE));

    // No delegators & delegated to self - returns balance.
    let b_weight = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V1,
      "B",
    );

    assert_eq!(b_weight.unwrap(), Decimal::new(1000000000, LEDGER_BALANCE_SCALE));

    // Delegated to self & has delegators - returns balance + delegators.
    let a_weight = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V1,
      "A",
    );
    assert_eq!(a_weight.unwrap(), Decimal::new(3000000000, LEDGER_BALANCE_SCALE));
  }

  #[test]
  fn test_stake_weight_v2() {
    let (a, b, c, d, e) = get_accounts();
    let map = get_votes();

    // No account found - throw err.
    let error = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone(), e.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V2,
      "F",
    );
    assert!(error.is_err());

    let a_weight = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone(), e.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V2,
      "A",
    );
    assert_eq!(a_weight.unwrap(), Decimal::new(2000000000, LEDGER_BALANCE_SCALE));

    let b_weight = Ledger::get_stake_weight(
      &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone(), e.clone()]),
      &Wrapper(map.clone()),
      &ProposalVersion::V2,
      "B",
    );

    assert_eq!(b_weight.unwrap(), Decimal::new(2000000000, LEDGER_BALANCE_SCALE));
  }

  fn get_accounts() -> (LedgerAccount, LedgerAccount, LedgerAccount, LedgerAccount, LedgerAccount) {
    (
      LedgerAccount::new("A".to_string(), "1".to_string(), None),
      LedgerAccount::new("B".to_string(), "1".to_string(), None),
      LedgerAccount::new("C".to_string(), "1".to_string(), Some("A".to_string())),
      LedgerAccount::new("D".to_string(), "1".to_string(), Some("A".to_string())),
      LedgerAccount::new("E".to_string(), "1".to_string(), Some("B".to_string())),
    )
  }

  fn get_votes() -> HashMap<String, Vote> {
    let mut map = HashMap::new();
    map.insert("B".to_string(), Vote::new("B".to_string(), "", "", 1, BlockStatus::Canonical, 1, 0));
    map.insert("C".to_string(), Vote::new("C".to_string(), "", "", 1, BlockStatus::Canonical, 1, 0));
    map
  }
}
