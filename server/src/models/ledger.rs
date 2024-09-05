use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::anyhow;
use anyhow::Context;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::config::NetworkConfig;
use crate::models::diesel::ProposalVersion;
use crate::models::vote::MinaVote;
use crate::prelude::*;

const LEDGER_BALANCE_SCALE: u32 = 9;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Ledger(pub(crate) Vec<LedgerAccount>);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LedgerAccount {
    pub(crate) pk: String,
    pub(crate) balance: String,
    pub(crate) delegate: String,
}

// let url = "https://673156464838-mina-staking-ledgers.s3.us-west-2.amazonaws.com/mainnet/mainnet-74-jxvumaCvujr7UzW1qCB87YR2RWu8CqvkwrCmHY8kkwpvN4WbTJn.json.tar.gz";

impl Ledger {
    pub(crate) async fn fetch(
        hash: impl Into<String>,
        ledger_storage_path: String,
        network: NetworkConfig,
        bucket_name: String,
        epoch: u16,
    ) -> Result<Ledger> {
        let hash: String = hash.into();
        let ledger_path_raw = f!("{ledger_storage_path}/{hash}");
        let ledger_path = Path::new(&ledger_path_raw);
        if !ledger_path.exists() {
            let url = f!("https://{bucket_name}.s3.us-west-2.amazonaws.com/{network}/{network}-{epoch}-{hash}.json.tar.gz");
            let response = reqwest::get(url).await.unwrap();
            if response.status().is_success() {
                // Get the object body as bytes
                let body = response.bytes().await.unwrap();
                let tar_gz = flate2::read::GzDecoder::new(&body[..]);
                let mut archive = tar::Archive::new(tar_gz);
                std::fs::create_dir_all(ledger_path).unwrap();
                archive.unpack(ledger_path).unwrap();
            }
        }
        let mut bytes = Vec::new();
        let ledger_path_str = ledger_path.to_str().unwrap();
        std::fs::File::open(f!("{ledger_path_str}/{network}-{epoch}-{hash}.json"))
            .with_context(|| f!("failed to open ledger {hash}"))?
            .read_to_end(&mut bytes)
            .with_context(|| f!("failed to read ledger {hash}"))?;
        Ok(Ledger(serde_json::from_slice(&bytes).with_context(
            || f!("failed to deserialize ledger {hash}"),
        )?))
    }

    pub(crate) fn get_stake_weight(
        &self,
        map: &Wrapper<HashMap<String, MinaVote>>,
        version: &ProposalVersion,
        public_key: impl Into<String>,
    ) -> Result<Decimal> {
        let public_key = public_key.into();

        let account = self
            .0
            .iter()
            .find(|d| d.pk == public_key)
            .ok_or_else(|| anyhow!("account {public_key} not found in ledger"))?;

        let balance = account
            .balance
            .parse()
            .unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE));

        match version {
            ProposalVersion::V1 => {
                if account.delegate != public_key {
                    return Ok(Decimal::new(0, LEDGER_BALANCE_SCALE));
                }

                let delegators = self
                    .0
                    .iter()
                    .filter(|d| d.delegate == public_key && d.pk != public_key)
                    .collect::<Vec<&LedgerAccount>>();

                if delegators.is_empty() {
                    return Ok(balance);
                }

                let stake_weight =
                    delegators
                        .iter()
                        .fold(Decimal::new(0, LEDGER_BALANCE_SCALE), |acc, x| {
                            x.balance
                                .parse()
                                .unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE))
                                + acc
                        });

                Ok(stake_weight + balance)
            }
            ProposalVersion::V2 => {
                let delegators = self
                    .0
                    .iter()
                    .filter(|d| {
                        d.delegate == public_key && d.pk != public_key && !map.0.contains_key(&d.pk)
                    })
                    .collect::<Vec<&LedgerAccount>>();

                if delegators.is_empty() {
                    return Ok(balance);
                }

                let stake_weight =
                    delegators
                        .iter()
                        .fold(Decimal::new(0, LEDGER_BALANCE_SCALE), |acc, x| {
                            x.balance
                                .parse()
                                .unwrap_or_else(|_| Decimal::new(0, LEDGER_BALANCE_SCALE))
                                + acc
                        });

                Ok(stake_weight + balance)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl LedgerAccount {
        pub fn new(pk: String, balance: String, delegate: String) -> LedgerAccount {
            LedgerAccount {
                pk,
                balance,
                delegate,
            }
        }
    }

    fn get_accounts() -> (
        LedgerAccount,
        LedgerAccount,
        LedgerAccount,
        LedgerAccount,
        LedgerAccount,
    ) {
        return (
            LedgerAccount::new("A".to_string(), "1".to_string(), "A".to_string()),
            LedgerAccount::new("B".to_string(), "1".to_string(), "B".to_string()),
            LedgerAccount::new("C".to_string(), "1".to_string(), "A".to_string()),
            LedgerAccount::new("D".to_string(), "1".to_string(), "A".to_string()),
            LedgerAccount::new("E".to_string(), "1".to_string(), "B".to_string()),
        );
    }

    fn get_votes() -> HashMap<String, MinaVote> {
        let mut map = HashMap::new();
        map.insert(
            "B".to_string(),
            MinaVote::new(
                "B".to_string(),
                "",
                "",
                1,
                crate::models::vote::MinaBlockStatus::Canonical,
                1,
                0,
            ),
        );

        map.insert(
            "C".to_string(),
            MinaVote::new(
                "C".to_string(),
                "",
                "",
                1,
                crate::models::vote::MinaBlockStatus::Canonical,
                1,
                0,
            ),
        );

        map
    }

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
        assert_eq!(error.is_err(), true);

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

        assert_eq!(
            b_weight.unwrap(),
            Decimal::new(1000000000, LEDGER_BALANCE_SCALE)
        );

        // Delegated to self & has delegators - returns balance + delegators.
        let a_weight = Ledger::get_stake_weight(
            &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone()]),
            &Wrapper(map.clone()),
            &ProposalVersion::V1,
            "A",
        );
        assert_eq!(
            a_weight.unwrap(),
            Decimal::new(3000000000, LEDGER_BALANCE_SCALE)
        );
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
        assert_eq!(error.is_err(), true);

        let a_weight = Ledger::get_stake_weight(
            &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone(), e.clone()]),
            &Wrapper(map.clone()),
            &ProposalVersion::V2,
            "A",
        );
        assert_eq!(
            a_weight.unwrap(),
            Decimal::new(2000000000, LEDGER_BALANCE_SCALE)
        );

        let b_weight = Ledger::get_stake_weight(
            &Ledger(vec![a.clone(), b.clone(), c.clone(), d.clone(), e.clone()]),
            &Wrapper(map.clone()),
            &ProposalVersion::V2,
            "B",
        );

        assert_eq!(
            b_weight.unwrap(),
            Decimal::new(2000000000, LEDGER_BALANCE_SCALE)
        );
    }
}
