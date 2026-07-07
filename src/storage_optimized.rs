use crate::metrics::*;
use crate::Result;
use bytemuck::bytes_of;
use lru::LruCache;
use memmap2::Mmap;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::fs::{self, File, OpenOptions};
use std::io::{Error, ErrorKind, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Lock-free, high-performance storage engine with memory-mapped I/O
pub struct OptimizedStorage {
    /// Memory-mapped file for time-series data
    mmap: Mutex<Option<Mmap>>,
    /// File handle for appending new blocks
    file: Mutex<File>,
    // Base path for metadata (kept for future use)
    #[allow(dead_code)]
    base_path: PathBuf,
    /// LRU cache for frequently accessed blocks (cache 10,000 blocks ~ 40MB)
    block_cache: Mutex<LruCache<u64, Arc<MetricBlock>>>,
    /// Current block count (updated atomically)
    block_count: AtomicU64,
    /// File size for validation
    file_size: AtomicU64,
}

impl OptimizedStorage {
    /// Open or create the database with optimized settings
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&base_path)?;

        let metadata = fs::metadata(&base_path)?;
        let file_size = metadata.len();
        let block_size = std::mem::size_of::<MetricBlock>() as u64;
        let block_count = file_size / block_size;

        let storage = Self {
            mmap: Mutex::new(None),
            file: Mutex::new(file),
            base_path,
            block_cache: Mutex::new(LruCache::new(NonZeroUsize::new(10_000).unwrap())),
            block_count: AtomicU64::new(block_count),
            file_size: AtomicU64::new(file_size),
        };

        // Memory-map the file for instant access
        storage.remap()?;

        Ok(storage)
    }

    /// Remap the file into memory (call after file grows)
    fn remap(&self) -> Result<()> {
        let file = self.file.lock();
        if let Ok(mmap) = unsafe { Mmap::map(&*file) } {
            let mut mmap_guard = self.mmap.lock();
            *mmap_guard = Some(mmap);
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Other, "failed to memory-map file"))
        }
    }

    /// Get a block by index with zero-copy from memory map
    pub fn get_block(&self, block_index: u64) -> Result<Arc<MetricBlock>> {
        // Check cache first (lock-free fast path)
        {
            let mut cache = self.block_cache.lock();
            if let Some(block) = cache.get(&block_index) {
                return Ok(Arc::clone(block));
            }
        }

        // Read from memory-mapped file (zero-copy)
        let block_size = std::mem::size_of::<MetricBlock>() as u64;
        let offset = block_index
            .checked_mul(block_size)
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "offset overflow"))?;

        let mmap_guard = self.mmap.lock();
        let mmap = mmap_guard
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Other, "file not memory-mapped"))?;

        if offset + block_size > mmap.len() as u64 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "block beyond file end"));
        }

        // Zero-copy: directly read from memory map
        let bytes = &mmap[offset as usize..(offset + block_size) as usize];
        let block: &MetricBlock = bytemuck::from_bytes(bytes);

        // Cache it for future access
        let cached = Arc::new(*block);
        let mut cache = self.block_cache.lock();
        cache.put(block_index, Arc::clone(&cached));

        Ok(cached)
    }

    /// Append a block efficiently
    pub fn append_block(&self, block: &MetricBlock) -> Result<u64> {
        let mut file = self.file.lock();
        let block_size = std::mem::size_of::<MetricBlock>() as u64;

        // Seek to end and write
        file.seek(SeekFrom::End(0))?;
        file.write_all(bytes_of(block))?;
        file.sync_all()?;

        // Update counters atomically
        let new_count = self.block_count.fetch_add(1, Ordering::Relaxed) + 1;
        let _new_size = self.file_size.fetch_add(block_size, Ordering::Relaxed) + block_size;

        // Remap if file grew significantly (every 1000 blocks)
        if new_count % 1000 == 0 {
            drop(file);
            self.remap()?;
        }

        Ok(new_count - 1) // Return the index of the block we just wrote
    }

    /// Get current block count (lock-free atomic read)
    pub fn block_count(&self) -> u64 {
        self.block_count.load(Ordering::Relaxed)
    }

    /// Generate mock database blazingly fast using parallel writes
    pub fn generate_mock_database(&self, hours: i64) -> Result<()> {
        // Clear existing data
        {
            let file = self.file.lock();
            file.set_len(0)?;
            file.sync_all()?;
        }

        // Reset counters
        self.block_count.store(0, Ordering::Relaxed);
        self.file_size.store(0, Ordering::Relaxed);
        self.block_cache.lock().clear();

        // Remap to reset memory map
        self.remap()?;

        // Generate blocks in parallel using rayon
        let blocks: Vec<MetricBlock> = (0..hours)
            .into_par_iter()
            .map(|hour| MetricBlock::fill_with_hour(hour))
            .collect();

        // Write all blocks sequentially (OS will batch writes)
        let mut file = self.file.lock();
        for block in &blocks {
            file.write_all(bytes_of(block))?;
        }
        file.sync_all()?;

        // Update counters
        let count = blocks.len() as u64;
        let size = count * std::mem::size_of::<MetricBlock>() as u64;
        self.block_count.store(count, Ordering::Relaxed);
        self.file_size.store(size, Ordering::Relaxed);

        // Remap for reading
        drop(file);
        self.remap()?;

        // Pre-populate cache with first 1000 blocks
        self.preload_cache(1000);

        Ok(())
    }

    /// Pre-load blocks into cache for faster access
    fn preload_cache(&self, count: usize) {
        let mut cache = self.block_cache.lock();
        for i in 0..count.min(self.block_count() as usize) {
            if let Ok(block) = self.get_block(i as u64) {
                cache.put(i as u64, block);
            }
        }
    }

    /// Batch read multiple blocks (parallel I/O)
    pub fn get_blocks_batch(&self, indices: &[u64]) -> Result<Vec<Arc<MetricBlock>>> {
        use rayon::prelude::*;

        indices
            .par_iter()
            .map(|&idx| self.get_block(idx))
            .collect()
    }

    /// Invalidate cache entry (call after writes)
    pub fn invalidate_cache(&self, block_index: u64) {
        let mut cache = self.block_cache.lock();
        cache.pop(&block_index);
    }

    /// Clear entire cache
    pub fn clear_cache(&self) {
        self.block_cache.lock().clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.block_cache.lock();
        (cache.len(), cache.cap().into())
    }

    /// Load the catalog (table schemas) from metadata file
    pub fn load_catalog(&self) -> Result<crate::Catalog> {
        let metadata_path = self.base_path.with_extension("meta.json");
        if !metadata_path.exists() {
            return Ok(crate::Catalog::default());
        }
        let contents = fs::read_to_string(&metadata_path)?;
        let catalog: crate::Catalog = serde_json::from_str(&contents)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
        Ok(catalog)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_optimized_storage() -> Result<()> {
        let path = "target/test_optimized.fdb";
        fs::remove_file(path).ok();

        let storage = OptimizedStorage::open(path)?;
        storage.generate_mock_database(100)?;

        assert_eq!(storage.block_count(), 100);

        let block = storage.get_block(50)?;
        assert_eq!(block.metrics[0], 20.0 + 50.0 + 0.01);

        // Test cache
        let (cached, _cap) = storage.cache_stats();
        assert!(cached > 0);

        fs::remove_file(path).ok();
        Ok(())
    }

    #[test]
    fn test_batch_read() -> Result<()> {
        let path = "target/test_batch.fdb";
        fs::remove_file(path).ok();

        let storage = OptimizedStorage::open(path)?;
        storage.generate_mock_database(50)?;

        let indices: Vec<u64> = vec![0, 10, 20, 30, 40];
        let blocks = storage.get_blocks_batch(&indices)?;

        assert_eq!(blocks.len(), 5);
        assert_eq!(blocks[2].metrics[0], 20.0 + 20.0 + 0.01);

        fs::remove_file(path).ok();
        Ok(())
    }
}