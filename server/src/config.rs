use crate::{cache::CacheManager, prelude::*, MinaProposalManifest};
use axum::http::{HeaderValue, Method};
use clap::{Parser, ValueEnum};
use diesel::{r2d2::ConnectionManager, PgConnection};
use r2d2::Pool;
use std::{collections::HashSet, fmt, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

pub type ConnManager = Pool<ConnectionManager<PgConnection>>;

#[derive(Clone)]
pub struct Context {
  pub cache: Arc<CacheManager>,
  pub conn_manager: Arc<ConnManager>,
  pub network: NetworkConfig,
  pub ledger_storage_path: String,
  pub bucket_name: String,
  pub manifest: MinaProposalManifest,
}

#[derive(Clone, Copy, Parser, ValueEnum, Debug)]
pub enum NetworkConfig {
  Mainnet,
  Devnet,
  Berkeley,
}

impl fmt::Display for NetworkConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      NetworkConfig::Mainnet => write!(f, "mainnet"),
      NetworkConfig::Devnet => write!(f, "devnet"),
      NetworkConfig::Berkeley => write!(f, "berkeley"),
    }
  }
}

#[derive(Clone, Parser)]
pub struct Config {
  /// The mina network to connect to.
  #[clap(long, env)]
  pub mina_network: NetworkConfig,
  /// The connection URL for the archive database.
  #[clap(long, env)]
  pub archive_database_url: String,
  /// API Port.
  #[clap(long, env, default_value_t = 8080)]
  pub port: u16,
  /// Origins allowed to make cross-site requests.
  #[clap(long, env = "SERVER_ALLOWED_ORIGINS", value_parser = parse_allowed_origins )]
  pub allowed_origins: HashSet<String>,
  /// Set the name of the bucket containing the ledgers
  #[clap(long, env)]
  pub bucket_name: String,
  /// Path to store the ledgers
  #[clap(long, env, default_value = "/tmp/ledgers")]
  pub ledger_storage_path: String,
}

#[allow(clippy::unnecessary_wraps)]
fn parse_allowed_origins(arg: &str) -> Result<HashSet<String>> {
  let allowed_origins =
    HashSet::from_iter(arg.split_whitespace().map(std::borrow::ToOwned::to_owned).collect::<HashSet<_>>());

  assert!(!allowed_origins.is_empty(), "failed to parse allowed_origins: {allowed_origins:?}");

  Ok(allowed_origins)
}

pub fn init_cors(cfg: &Config) -> CorsLayer {
  let origins = cfg
    .allowed_origins
    .clone()
    .into_iter()
    .map(|origin| origin.parse().unwrap_or_else(|_| panic!("Error: failed parsing allowed-origin {origin}")))
    .collect::<Vec<HeaderValue>>();

  let layer = if cfg.allowed_origins.contains("*") {
    CorsLayer::new().allow_origin(Any)
  } else {
    CorsLayer::new().allow_origin(origins)
  };

  layer.allow_methods([Method::GET, Method::POST, Method::OPTIONS]).allow_headers(Any)
}

pub fn init_tracing() {
  tracing_subscriber::fmt::Subscriber::builder()
    .with_env_filter(EnvFilter::from_default_env())
    .with_writer(std::io::stderr)
    .compact()
    .init();
}
