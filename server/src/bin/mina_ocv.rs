use anyhow::Result;
use clap::Parser;
use mina_ocv::ServeArgs;

#[tokio::main]
async fn main() -> Result<()> {
  ServeArgs::parse().serve().await
}
