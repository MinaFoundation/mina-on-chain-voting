use crate::{BlockStatus, ChainStatusType};
use anyhow::{Context, Result};
use diesel::{
  r2d2::ConnectionManager,
  sql_query,
  sql_types::{BigInt, Text},
  PgConnection, QueryableByName, RunQueryDsl,
};
use r2d2::Pool;

#[derive(Clone)]
pub struct Archive(Pool<ConnectionManager<PgConnection>>);

impl Archive {
  pub fn new(archive_database_url: &String) -> Self {
    let archive_manager = ConnectionManager::<PgConnection>::new(archive_database_url);
    let pool = Pool::builder()
      .test_on_check_out(true)
      .build(archive_manager)
      .unwrap_or_else(|_| panic!("Error: failed to build `archive` connection pool"));
    Self(pool)
  }

  pub fn fetch_chain_tip(&self) -> Result<i64> {
    let connection = &mut self.0.get().context("failed to get archive db connection")?;
    let result = sql_query("SELECT MAX(height) FROM blocks").get_result::<FetchChainTipResult>(connection)?;
    Ok(result.max)
  }

  pub fn fetch_latest_slot(&self) -> Result<i64> {
    let connection = &mut self.0.get().context("failed to get archive db connection")?;
    let result = sql_query("SELECT MAX(global_slot) FROM blocks").get_result::<FetchLatestSlotResult>(connection)?;
    Ok(result.max)
  }

  pub fn fetch_transactions(&self, start_time: i64, end_time: i64) -> Result<Vec<FetchTransactionResult>> {
    let connection = &mut self.0.get().context("failed to get archive db connection")?;
    let results = sql_query(
      "SELECT DISTINCT pk.value as account, uc.memo as memo, uc.nonce as nonce, uc.hash as hash, b.height as height, b.chain_status as status, b.timestamp::bigint as timestamp
      FROM user_commands AS uc
      JOIN blocks_user_commands AS buc
      ON uc.id = buc.user_command_id
      JOIN blocks AS b
      ON buc.block_id = b.id
      JOIN public_keys AS pk
      ON uc.source_id = pk.id
      WHERE uc.command_type = 'payment'
      AND uc.source_id = uc.receiver_id
      AND NOT b.chain_status = 'orphaned'
      AND buc.status = 'applied'
      AND b.timestamp::bigint BETWEEN $1 AND $2"
    );
    let results = results.bind::<BigInt, _>(start_time).bind::<BigInt, _>(end_time).get_results(connection)?;
    Ok(results)
  }
}

#[derive(QueryableByName)]
pub struct FetchChainTipResult {
  #[diesel(sql_type = BigInt)]
  pub max: i64,
}

#[derive(QueryableByName)]
pub struct FetchLatestSlotResult {
  #[diesel(sql_type = BigInt)]
  pub max: i64,
}

#[derive(QueryableByName)]
pub struct FetchTransactionResult {
  #[diesel(sql_type = Text)]
  pub account: String,
  #[diesel(sql_type = Text)]
  pub hash: String,
  #[diesel(sql_type = Text)]
  pub memo: String,
  #[diesel(sql_type = BigInt)]
  pub height: i64,
  #[diesel(sql_type = ChainStatusType)]
  pub status: BlockStatus,
  #[diesel(sql_type = BigInt)]
  pub timestamp: i64,
  #[diesel(sql_type = BigInt)]
  pub nonce: i64,
}
