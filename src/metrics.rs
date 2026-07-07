use bytemuck::{Pod, Zeroable};

/// The number of 32-bit metric samples stored inside each physical block.
pub const CHUNK_SIZE: usize = 1024;

/// Each block represents one hour of time-series data.
pub const CHUNK_DURATION_SEC: i64 = 3600;

/// Base timestamp used for constant O(1) block indexing.
pub const BASE_TIMESTAMP: i64 = 1_700_000_000;

/// Storage-aligned block of floating-point metrics.
#[derive(Copy, Clone, Debug)]
#[repr(C, align(4096))]
pub struct MetricBlock {
    pub metrics: [f32; CHUNK_SIZE],
}

unsafe impl Pod for MetricBlock {}
unsafe impl Zeroable for MetricBlock {}

impl MetricBlock {
    /// Create a default block initialized to a single value.
    pub fn new(value: f32) -> Self {
        Self {
            metrics: [value; CHUNK_SIZE],
        }
    }

    /// Generate a deterministic hourly block for testing and mock data.
    pub fn fill_with_hour(hour: i64) -> Self {
        let mut metrics = [0.0_f32; CHUNK_SIZE];
        for i in 0..CHUNK_SIZE {
            metrics[i] = 20.0 + (hour as f32) + ((i as f32) * 0.01);
        }
        Self { metrics }
    }
}
