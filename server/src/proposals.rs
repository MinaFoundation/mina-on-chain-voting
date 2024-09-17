use bytes::Bytes;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProposalVersion {
  V1,
  V2,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProposalCategory {
  Core,
  Networking,
  Interface,
  ERC,
  Cryptography,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MinaProposal {
  pub id: usize,
  pub key: String,
  pub start_time: i64,
  pub end_time: i64,
  pub epoch: i64,
  pub ledger_hash: Option<String>,
  pub category: ProposalCategory,
  pub version: ProposalVersion,
  pub title: String,
  pub description: String,
  pub url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MinaProposalManifest {
  pub proposals: Vec<MinaProposal>,
}

static PROPOSALS_MANIFEST_GITHUB_URL: &str =
  "https://raw.githubusercontent.com/MinaFoundation/mina-on-chain-voting/main/server/proposals/proposals.json";

impl MinaProposalManifest {
  // TODO: error messages
  pub async fn load(maybe_proposals_url: &Option<String>) -> Result<Self> {
    let manifest_bytes = match maybe_proposals_url {
      Some(url) => {
        let url = if url.is_empty() { &PROPOSALS_MANIFEST_GITHUB_URL.to_string() } else { url };
        reqwest::Client::new().get(url).send().await?.bytes().await?
      }
      None => Bytes::from_static(include_bytes!("../proposals/proposals.json")),
    };
    Ok(serde_json::from_slice(manifest_bytes.as_ref())?)
  }

  pub fn proposal(&self, id: usize) -> Result<MinaProposal> {
    self.proposals.get(id).cloned().ok_or(anyhow!("Could not retrieve proposal with ID {}", id))
  }
}
