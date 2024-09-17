use crate::{cache::CacheManager, MinaProposalManifest, NetworkConfig};
use anyhow::Result;
use clap::Args;
use diesel::{r2d2::ConnectionManager, PgConnection};
use r2d2::Pool;
use std::sync::Arc;

pub type ConnManager = Pool<ConnectionManager<PgConnection>>;

#[derive(Clone)]
pub struct Ocv {
  pub cache: Arc<CacheManager>,
  pub conn_manager: Arc<ConnManager>,
  pub network: NetworkConfig,
  pub ledger_storage_path: String,
  pub bucket_name: String,
  pub manifest: MinaProposalManifest,
}

#[derive(Clone, Args)]
pub struct OcvConfig {
  /// The mina network to connect to.
  #[clap(long, env)]
  pub mina_network: NetworkConfig,
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
    let cache = CacheManager::build();
    let manifest = MinaProposalManifest::load(&self.maybe_proposals_url).await?;
    let archive_manager = ConnectionManager::<PgConnection>::new(&self.archive_database_url);
    let conn_manager = Pool::builder()
      .test_on_check_out(true)
      .build(archive_manager)
      .unwrap_or_else(|_| panic!("Error: failed to build `archive` connection pool"));
    Ok(Ocv {
      cache: Arc::new(cache),
      conn_manager: Arc::new(conn_manager),
      network: self.mina_network,
      ledger_storage_path: self.ledger_storage_path.clone(),
      bucket_name: self.bucket_name.clone(),
      manifest,
    })
  }
}
