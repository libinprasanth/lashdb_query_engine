use flashdb_query_engine::{aggregate_sum_simd, run_tcp_server, run_optimized_tcp_server, EngineStorage, WebUI, BASE_TIMESTAMP, CHUNK_DURATION_SEC};
use parking_lot::RwLock;
use std::env;
use std::sync::Arc;
use std::time::Instant;

fn print_usage(program: &str) {
    println!("Usage: {} <command> [args]", program);
    println!("Commands:");
    println!("  demo              generate data and run a local query");
    println!("  serve [addr]      start a TCP server (default 127.0.0.1:4000)");
    println!("  serve-fast [addr] start OPTIMIZED multi-threaded server (default 127.0.0.1:4000)");
    println!("  web [port]        start web UI server (default 8080)");
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let program = args.get(0).map(|s| s.as_str()).unwrap_or("flashdb_query_engine");

    match args.get(1).map(|s| s.as_str()) {
        Some("serve") => {
            let addr = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1:4000");
            run_tcp_server("hardware_native.fdb", addr)
        }
        Some("serve-fast") => {
            let addr = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1:4000");
            let workers = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            run_optimized_tcp_server("hardware_native.fdb", addr, workers)
        }
        Some("web") => {
            let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8080);
            let engine = EngineStorage::open("hardware_native.fdb")?;
            let engine = Arc::new(RwLock::new(engine));
            WebUI::new(engine, port).run()
        }
        Some("demo") | None => run_demo(),
        _ => {
            print_usage(program);
            Ok(())
        }
    }
}

fn run_demo() -> std::io::Result<()> {
    let db_path = "hardware_native.fdb";
    let mut engine = EngineStorage::open(db_path)?;

    println!("1. Pre-allocating 1,000 hours of continuous data matrix onto disk...");
    engine.generate_mock_database(1000)?;

    let target_query_time = BASE_TIMESTAMP + (500 * CHUNK_DURATION_SEC) + 1800; // Hour 500, 30 min mark
    println!("2. Executing O(1) instant seek lookup for timestamp: {}", target_query_time);

    let start_seek = Instant::now();
    let block = engine.read_block_at_time(target_query_time)?;
    let seek_duration = start_seek.elapsed();
    println!(" -> Block retrieved from physical storage in: {:?}", seek_duration);

    println!("3. Running SIMD-style aggregation on loaded block...");
    let start_simd = Instant::now();
    let sum_total = aggregate_sum_simd(&block);
    let simd_duration = start_simd.elapsed();
    println!(" -> Compiled mathematical metrics total: {}", sum_total);
    println!(" -> Aggregation completed in: {:?}", simd_duration);

    let stored_blocks = engine.block_count()?;
    println!("Stored blocks on disk: {}", stored_blocks);

    std::fs::remove_file(db_path).ok();
    Ok(())
}
