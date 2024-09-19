use crate::{Archive, Caches, Ocv, ProposalsManifest};
use anyhow::Result;
use clap::{Args, Parser, ValueEnum};
use std::{fmt, fs, path::PathBuf, str::FromStr};

#[derive(Clone, Args)]
pub struct OcvConfig {
  /// The mina network to connect to.
  #[clap(long, env)]
  pub mina_network: Network,
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
      network: self.mina_network,
      ledger_storage_path: PathBuf::from_str(&self.ledger_storage_path)?,
      bucket_name: self.bucket_name.clone(),
      proposals_manifest: ProposalsManifest::load(&self.maybe_proposals_url).await?,
    })
  }
}

#[derive(Clone, Copy, Parser, ValueEnum, Debug)]
pub enum Network {
  Mainnet,
  Devnet,
  Berkeley,
}

impl fmt::Display for Network {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Network::Mainnet => write!(f, "mainnet"),
      Network::Devnet => write!(f, "devnet"),
      Network::Berkeley => write!(f, "berkeley"),
    }
  }
}
