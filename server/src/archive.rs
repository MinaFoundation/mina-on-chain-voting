use anyhow::{Context, Result};
use diesel::{
  PgConnection, QueryableByName, RunQueryDsl,
  r2d2::ConnectionManager,
  sql_query,
  sql_types::{BigInt, Text},
};
use r2d2::Pool;

use crate::{BlockStatus, ChainStatusType};

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

pub trait ArchiveInterface {
  fn fetch_chain_tip(&self) -> Result<i64>;
  fn fetch_latest_slot(&self) -> Result<i64>;
  fn fetch_transactions(&self, start_time: i64, end_time: i64) -> Result<Vec<FetchTransactionResult>>;
}


impl ArchiveInterface for Archive {
  fn fetch_chain_tip(&self) -> Result<i64> {
      self.fetch_chain_tip()
  }

  fn fetch_latest_slot(&self) -> Result<i64> {
      self.fetch_latest_slot()
  }

  fn fetch_transactions(&self, start_time: i64, end_time: i64) -> Result<Vec<FetchTransactionResult>> {
      self.fetch_transactions(start_time, end_time)
  }
}


pub struct MockArchive;

impl ArchiveInterface for MockArchive {
    fn fetch_chain_tip(&self) -> Result<i64> {
        Ok(100) // Return a mock value for the chain tip
    }

    fn fetch_latest_slot(&self) -> Result<i64> {
        Ok(200) // Return a mock value for the latest slot
    }

    fn fetch_transactions(&self, start_time: i64, end_time: i64) -> Result<Vec<FetchTransactionResult>> {
        Ok(vec![
            FetchTransactionResult {
                account: "mock_account".to_string(),
                hash: "mock_hash".to_string(),
                memo: "mock_memo".to_string(),
                height: 1,
                status: BlockStatus::Pending, // Use a mock value
                timestamp: start_time + 1000,
                nonce: 42,
            },
        ]) // Return a mock list of transactions
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_chain_tip() {
        let archive = MockArchive;
        let chain_tip = archive.fetch_chain_tip().unwrap();
        assert_eq!(chain_tip, 100);
    }

    #[test]
    fn test_fetch_latest_slot() {
        let archive = MockArchive;
        let latest_slot = archive.fetch_latest_slot().unwrap();
        assert_eq!(latest_slot, 200);
    }

    #[test]
    fn test_fetch_transactions() {
        let archive = MockArchive;
        let transactions = archive.fetch_transactions(1733371364000, 1733803364000).unwrap();
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].account, "mock_account");
    }
}
