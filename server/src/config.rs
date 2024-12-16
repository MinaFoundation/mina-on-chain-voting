use std::{fs, path::PathBuf, str::FromStr};

use anyhow::Result;
use bytes::Bytes;
use clap::{Args, Parser, ValueEnum};
use derive_more::Display;
use serde::{Deserialize, Serialize};

use crate::{Archive, Caches, Ocv, Proposal, ProposalsManifest};

#[derive(Clone, Args)]
pub struct OcvConfig {
  /// The mina network to connect to.
  #[clap(long, env)]
  pub network: Network,
  /// The environment stage.
  #[clap(long, env = "RELEASE_STAGE")]
  pub release_stage: ReleaseStage,
  /// The URL from which the `proposals.json` should be fetched.
  #[clap(long, env = "PROPOSALS_URL")]
  pub maybe_proposals_url: Option<String>,
  /// The connection URL for the archive database.
  #[clap(long, env)]
  pub archive_database_url: String,
  /// Set the name of the bucket containing the ledgers
  #[clap(long, env)]
  pub bucket_name: String,
  /// Path to store the ledgers
  #[clap(long, env, default_value = "/tmp/ledgers")]
  pub ledger_storage_path: String,
}

impl OcvConfig {
  pub async fn to_ocv(&self) -> Result<Ocv> {
    fs::create_dir_all(&self.ledger_storage_path)?;
    Ok(Ocv {
      caches: Caches::build(),
      archive: Archive::new(&self.archive_database_url),
      network: self.network,
      release_stage: self.release_stage,
      ledger_storage_path: PathBuf::from_str(&self.ledger_storage_path)?,
      bucket_name: self.bucket_name.clone(),
      proposals: self.load_proposals().await?,
    })
  }

  async fn load_proposals(&self) -> Result<Vec<Proposal>> {
    let manifest_bytes = match &self.maybe_proposals_url {
      Some(url) => {
        let url = if url.is_empty() { &PROPOSALS_MANIFEST_GITHUB_URL.to_string() } else { url };
        reqwest::Client::new().get(url).send().await?.bytes().await?
      }
      None => Bytes::from_static(include_bytes!("../proposals/proposals.json")),
    };
    let manifest: ProposalsManifest = serde_json::from_slice(manifest_bytes.as_ref())?;
    let filtered_by_network =
      manifest.proposals.into_iter().filter(|proposal| proposal.network == self.network).collect();
    Ok(filtered_by_network)
  }
}

static PROPOSALS_MANIFEST_GITHUB_URL: &str =
  "https://raw.githubusercontent.com/MinaFoundation/mina-on-chain-voting/main/server/proposals/proposals.json";

#[derive(Clone, Copy, Parser, ValueEnum, Debug, Display, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Network {
  #[display("mainnet")]
  Mainnet,
  #[display("devnet")]
  Devnet,
  #[display("berkeley")]
  Berkeley,
}

#[derive(Clone, Copy, Parser, ValueEnum, Debug, Display, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseStage {
  #[display("development")]
  Development,
  #[display("staging")]
  Staging,
  #[display("production")]
  Production,
}
