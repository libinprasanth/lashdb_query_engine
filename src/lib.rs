pub mod metrics;
pub mod query;
pub mod server;
pub mod server_optimized;
pub mod sql;
pub mod storage;
pub mod storage_optimized;
pub mod web;

pub use metrics::*;
pub use query::*;
pub use server::*;
pub use server_optimized::*;
pub use sql::*;
pub use storage::*;
pub use storage_optimized::*;
pub use web::*;

/// A unified result type for file-backed database operations.
pub type Result<T> = std::result::Result<T, std::io::Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const TEST_DB_PATH: &str = "target/test_flashdb.fdb";

    #[test]
    fn metric_block_is_4096_bytes() {
        assert_eq!(std::mem::size_of::<MetricBlock>(), 4096);
    }

    #[test]
    fn generate_and_read_block_roundtrip() -> Result<()> {
        fs::remove_file(TEST_DB_PATH).ok();
        let mut engine = EngineStorage::open(TEST_DB_PATH)?;
        engine.generate_mock_database(2)?;

        let block = engine.read_block_at_time(BASE_TIMESTAMP + CHUNK_DURATION_SEC)?;
        assert_eq!(block.metrics[0], 21.0);
        assert_eq!(engine.block_count()?, 2);

        fs::remove_file(TEST_DB_PATH).ok();
        Ok(())
    }
}
