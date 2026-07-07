use crate::metrics::MetricBlock;

/// Compute a simple sum for a single fixed-size metric block.
pub fn aggregate_sum(block: &MetricBlock) -> f32 {
    block.metrics.iter().copied().sum()
}

/// Perform a register-friendly block sum that is easy for the compiler to auto-vectorize.
pub fn aggregate_sum_simd(block: &MetricBlock) -> f32 {
    let mut total = 0.0_f32;
    for chunk in block.metrics.chunks_exact(8) {
        let mut temp = [0.0_f32; 8];
        temp.copy_from_slice(chunk);
        total += temp.iter().sum::<f32>();
    }
    total
}

/// Sum a sequence of blocks.
pub fn aggregate_range_sum(blocks: &[MetricBlock]) -> f32 {
    blocks.iter().map(aggregate_sum).sum()
}
