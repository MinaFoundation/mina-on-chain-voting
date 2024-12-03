use std::sync::OnceLock;

use aws_sdk_s3::{
  Client,
  config::{Builder, Region},
};

pub fn s3_client() -> &'static Client {
  static HASHMAP: OnceLock<Client> = OnceLock::new();
  HASHMAP.get_or_init(|| {
    let region = Region::new("us-west-2");
    let config = Builder::new().region(region).behavior_version_latest().build();
    Client::from_conf(config)
  })
}
