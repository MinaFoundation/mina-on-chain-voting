use std::fmt;

use clap::{Parser, ValueEnum};

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
