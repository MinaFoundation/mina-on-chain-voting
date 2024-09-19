use anyhow::{anyhow, bail, Result};
use aws_sdk_s3::{
  config::{Builder, Region},
  operation::list_objects_v2::ListObjectsV2Output,
  types::Object,
  Client,
};
use flate2::read::GzDecoder;
use tar::Archive;

#[tokio::main]
async fn main() -> Result<()> {
  load("devnet".to_string(), 11).await?;
  Ok(())
}

static BUCKET_NAME: &str = "673156464838-mina-staking-ledgers";

async fn load(network: String, epoch: u16) -> Result<()> {
  let region = Region::new("us-west-2");
  let config = Builder::new().region(region).behavior_version_latest().build();
  let client = Client::from_conf(config);

  let ListObjectsV2Output { contents, .. } =
    client.list_objects_v2().bucket(BUCKET_NAME).prefix(format!("{network}/{network}-{epoch}")).send().await?;
  let Object { key: maybe_key, .. } =
    contents.and_then(|x| x.first().cloned()).ok_or(anyhow!("No such dump exists"))?;
  if let Some(key) = maybe_key {
    let bytes = client.get_object().bucket(BUCKET_NAME).key(key).send().await?.body.collect().await?.into_bytes();
    let tar_gz = GzDecoder::new(&bytes[..]);
    let mut archive = Archive::new(tar_gz);
    std::fs::create_dir_all("testing_dl")?;
    archive.unpack("testing_dl")?;
    Ok(())
  } else {
    bail!("Could not access filename vector");
  }
}
