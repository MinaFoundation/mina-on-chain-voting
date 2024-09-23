use serde::{Deserialize, Serialize};

use crate::Network;

#[derive(Deserialize, Debug, Clone)]
pub struct ProposalsManifest {
  pub proposals: Vec<Proposal>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proposal {
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
  pub network: Network,
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
pub enum ProposalVersion {
  V1,
  V2,
}
