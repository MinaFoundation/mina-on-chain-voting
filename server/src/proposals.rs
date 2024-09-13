use anyhow::{anyhow, Context, Result};
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
  pub id: i32,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MinaProposalManifest {
  pub proposals: Vec<MinaProposal>,
}

impl MinaProposalManifest {
  // TODO: error messages
  pub async fn load() -> Result<Self> {
    let manifest_bytes = reqwest::Client::new()
      .get("https://raw.githubusercontent.com/MinaFoundation/mina-on-chain-voting/main/proposals/proposals.json")
      .send()
      .await
      .context("TODO")?
      .bytes()
      .await
      .context("TODO")?;
    let manifest_slice = manifest_bytes.as_ref();
    serde_json::from_slice(manifest_slice).context("TODO")
  }

  pub fn proposal(&self, id: usize) -> Result<MinaProposal> {
    self.proposals.get(id).cloned().ok_or(anyhow!("Could not retrieve proposal with ID {}", id))
  }
}
