use aws_sdk_s3::{
  config::{Builder, Region},
  Client,
};
use std::sync::OnceLock;

pub fn s3_client() -> &'static Client {
  static HASHMAP: OnceLock<Client> = OnceLock::new();
  HASHMAP.get_or_init(|| {
    let region = Region::new("us-west-2");
    let config = Builder::new().region(region).behavior_version_latest().build();
    Client::from_conf(config)
  })
}
