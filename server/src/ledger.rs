use std::{collections::HashMap, fs, io::Read, path::PathBuf};

use anyhow::{Result, anyhow};
use flate2::read::GzDecoder;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tar::Archive;

use crate::{Ocv, ProposalVersion, Vote, Wrapper, s3_client};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ledger(pub Vec<LedgerAccount>);

impl Ledger {
  pub async fn fetch(ocv: &Ocv, hash: &String) -> Result<Ledger> {
    let dest = ocv.ledger_storage_path.join(format!("{hash}.json"));
    if !dest.exists() {
      Self::download(ocv, hash, &dest).await?;
    }
    let contents = fs::read(dest)?;
    Ok(Ledger(serde_json::from_slice(&contents[..]).expect("Expecting a valid list of ledger accounts.")))
  }

  async fn download(ocv: &Ocv, hash: &String, to: &PathBuf) -> Result<()> {
    let client = s3_client();
    let s3_path = client
      .list_objects_v2()
      .bucket(&ocv.bucket_name)
      .send()
      .await?
      .contents
      .and_then(|objects| {
        objects.into_iter().find(|object| object.key.as_ref().is_some_and(|key| key.contains(hash))).and_then(|x| x.key)
      })
      .ok_or(anyhow!("Could not retrieve dump corresponding to {hash}"))?;
    let bytes =
      client.get_object().bucket(&ocv.bucket_name).key(&s3_path).send().await?.body.collect().await?.into_bytes();
    let tar_gz = GzDecoder::new(&bytes[..]);
    let mut archive = Archive::new(tar_gz);
    for entry in archive.entries()? {
      let mut entry = entry?;
      let path = entry.path()?.to_str().expect("Expecting a valid path").to_owned();
      if s3_path.contains(&path) {
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)?;
        fs::write(to, buffer)?;
        break;
      }
    }
    Ok(())
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

  pub fn get_stake_weight_mep(
    &self,
    _map: &Wrapper<HashMap<String, Vote>>,
    public_key: impl Into<String>,
  ) -> Result<Decimal> {
    let public_key: String = public_key.into();

    let account =
      self.0.iter().find(|d| d.pk == public_key).ok_or_else(|| anyhow!("account {public_key} not found in ledger"))?;

    let balance = account.balance.parse().unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE));

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
