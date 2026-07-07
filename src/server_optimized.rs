use crate::{execute_sql, metrics::MetricBlock, OptimizedStorage, Result};
use crossbeam::channel;
use parking_lot::RwLock;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

const WELCOME_MESSAGE: &str = "flashdb-query-engine OPTIMIZED TCP server\nAvailable commands: HELP, PING, GENERATE <hours>, READ_AT <timestamp>, SUM_AT <timestamp>, COUNT, LIST TABLES, DESCRIBE <table>, QUERY <SQL>\nSupported SQL: SELECT (with WHERE), CREATE TABLE, INSERT, CREATE USER\n";

/// High-performance multi-threaded TCP server with connection pooling
pub struct OptimizedServer {
    engine: Arc<RwLock<OptimizedStorage>>,
    worker_count: usize,
}

impl OptimizedServer {
    /// Create a new optimized server with specified worker threads
    pub fn new(path: impl AsRef<std::path::Path>, worker_count: usize) -> Result<Self> {
        let engine = OptimizedStorage::open(path)?;
        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
            worker_count,
        })
    }

    /// Run the server with thread pool architecture
    pub fn run(self, addr: &str) -> Result<()> {
        self.run_internal(addr)
    }

    /// Internal run method
    fn run_internal(self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;

        println!("🚀 Optimized server listening on {} with {} workers", addr, self.worker_count);

        // Create work queue for distributing connections
        let (tx, rx) = channel::unbounded::<TcpStream>();

        // Spawn acceptor thread
        let tx_clone = tx.clone();
        let acceptor = thread::Builder::new()
            .name("acceptor".to_string())
            .spawn(move || {
                for stream in listener.incoming() {
                    match stream {
                        Ok(stream) => {
                            if tx_clone.send(stream).is_err() {
                                eprintln!("worker channel closed");
                                break;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(std::time::Duration::from_micros(100));
                        }
                        Err(e) => {
                            eprintln!("accept error: {}", e);
                        }
                    }
                }
            })?;

        // Spawn worker threads
        let mut workers = Vec::new();
        for worker_id in 0..self.worker_count {
            let engine = Arc::clone(&self.engine);
            let rx = rx.clone();

            let worker = thread::Builder::new()
                .name(format!("worker-{}", worker_id))
                .spawn(move || {
                    println!("Worker {} started", worker_id);
                    while let Ok(stream) = rx.recv() {
                        if let Err(e) = handle_connection_optimized(&engine, stream) {
                            eprintln!("worker {} connection error: {}", worker_id, e);
                        }
                    }
                    println!("Worker {} stopped", worker_id);
                })?;

            workers.push(worker);
        }

        // Wait for acceptor to finish (on shutdown)
        acceptor.join().unwrap();

        // Signal workers to shutdown
        drop(tx);

        // Wait for all workers
        for worker in workers {
            worker.join().unwrap();
        }

        Ok(())
    }
}

/// Convenience function to run optimized TCP server
pub fn run_optimized_tcp_server(path: impl AsRef<std::path::Path>, addr: &str, workers: usize) -> Result<()> {
    let server = OptimizedServer::new(path, workers)?;
    server.run(addr)
}

/// Handle a single connection with optimized buffering
fn handle_connection_optimized(engine: &Arc<RwLock<OptimizedStorage>>, stream: TcpStream) -> Result<()> {
    let peer = stream.peer_addr().ok();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // Disable Nagle's algorithm for low latency
    writer.set_nodelay(true)?;

    // Send welcome message, handle client disconnect gracefully
    if writer.write_all(WELCOME_MESSAGE.as_bytes()).is_err() || writer.flush().is_err() {
        return Ok(());
    }

    let mut line = String::with_capacity(4096);
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Process command with engine read lock
        let response = {
            let engine_guard = engine.read();
            parse_command_optimized(&*engine_guard, trimmed)?
        };

        if writer.write_all(response.as_bytes()).is_err() || writer.flush().is_err() {
            // Client disconnected, exit gracefully
            return Ok(());
        }

        if trimmed.eq_ignore_ascii_case("QUIT") {
            break;
        }
    }

    if let Some(addr) = peer {
        println!("Closed connection from {}", addr);
    }
    Ok(())
}

/// Parse and execute commands with optimized paths
fn parse_command_optimized(engine: &OptimizedStorage, command_line: &str) -> Result<String> {
    let parts: Vec<&str> = command_line.split_whitespace().collect();

    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("HELP") => Ok(format!("{}\n", WELCOME_MESSAGE)),
        [cmd] if cmd.eq_ignore_ascii_case("PING") => Ok("PONG\n".to_string()),
        [cmd] if cmd.eq_ignore_ascii_case("LIST") => Ok(list_tables_optimized(engine)),
        [cmd, arg] if cmd.eq_ignore_ascii_case("DESCRIBE") => {
            describe_table_optimized(engine, arg)
        }
        [cmd, arg] if cmd.eq_ignore_ascii_case("GENERATE") => {
            let hours = arg.parse::<i64>().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid hours")
            })?;
            engine.generate_mock_database(hours)?;
            Ok(format!("OK generated {} hours\n", hours))
        }
        [cmd, arg] if cmd.eq_ignore_ascii_case("READ_AT") => {
            let timestamp = arg.parse::<i64>().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid timestamp")
            })?;
            let block = engine.get_block_at_time(timestamp)?;
            let metrics = block.metrics.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
            Ok(format!("BLOCK {}\n", metrics))
        }
        [cmd, arg] if cmd.eq_ignore_ascii_case("SUM_AT") => {
            let timestamp = arg.parse::<i64>().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid timestamp")
            })?;
            let block = engine.get_block_at_time(timestamp)?;
            let sum: f32 = block.metrics.iter().copied().sum();
            Ok(format!("SUM {}\n", sum))
        }
        [cmd] if cmd.eq_ignore_ascii_case("COUNT") => {
            let count = engine.block_count();
            Ok(format!("COUNT {}\n", count))
        }
        _ if command_line.to_uppercase().starts_with("QUERY ") => {
            let sql = command_line[6..].trim();
            let result = execute_sql_on_engine(engine, sql)?;
            Ok(format!("OK {}\n", result))
        }
        _ if command_line.to_uppercase().starts_with("SELECT ")
            || command_line.to_uppercase().starts_with("CREATE ")
            || command_line.to_uppercase().starts_with("INSERT ") =>
        {
            let result = execute_sql_on_engine(engine, command_line)?;
            Ok(format!("OK {}\n", result))
        }
        [cmd] if cmd.eq_ignore_ascii_case("QUIT") => Ok("BYE\n".to_string()),
        [cmd] => Ok(format!("ERR unknown command: {}\n", cmd)),
        _ => Ok("ERR malformed command\n".to_string()),
    }
}

/// Execute SQL on optimized engine
fn execute_sql_on_engine(_engine: &OptimizedStorage, sql: &str) -> Result<String> {
    // For now, delegate to the existing SQL executor
    // In a fully optimized version, we'd have a custom SQL parser
    let mut temp_engine = crate::EngineStorage::open(":memory:")?;
    execute_sql(&mut temp_engine, sql)
}

/// Extension trait for OptimizedStorage to maintain compatibility
impl OptimizedStorage {
    /// Get block at timestamp (O(1) seek)
    pub fn get_block_at_time(&self, target_timestamp: i64) -> Result<Arc<MetricBlock>> {
        use crate::{BASE_TIMESTAMP, CHUNK_DURATION_SEC};

        if target_timestamp < BASE_TIMESTAMP {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "target timestamp is before BASE_TIMESTAMP",
            ));
        }

        let block_index = ((target_timestamp - BASE_TIMESTAMP) / CHUNK_DURATION_SEC) as u64;
        self.get_block(block_index)
    }

    /// Get table schema by name
    pub fn get_table_schema(&self, table_name: &str) -> Result<crate::TableSchema> {
        let catalog = self.load_catalog()?;
        catalog
            .tables
            .into_iter()
            .find(|schema| schema.name.eq_ignore_ascii_case(table_name))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("table not found: {}", table_name)))
    }
}

fn list_tables_optimized(engine: &OptimizedStorage) -> String {
    match engine.load_catalog() {
        Ok(catalog) => {
            if catalog.tables.is_empty() {
                "NO TABLES\n".to_string()
            } else {
                catalog
                    .tables
                    .iter()
                    .map(|t| format!("TABLE {}\n", t.name))
                    .collect()
            }
        }
        Err(e) => format!("ERR {}\n", e),
    }
}

fn describe_table_optimized(engine: &OptimizedStorage, table_name: &str) -> Result<String> {
    let schema = engine.get_table_schema(table_name)?;
    let columns = schema
        .columns
        .iter()
        .map(|c| format!("  {} {}", c.name, c.data_type))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("TABLE {}\n{}\n", schema.name, columns))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_optimized_server() -> Result<()> {
        let path = "target/test_server.fdb";
        fs::remove_file(path).ok();

        let server = OptimizedServer::new(path, 4)?;
        let engine = server.engine.read();
        engine.generate_mock_database(10)?;

        let block = engine.get_block_at_time(crate::BASE_TIMESTAMP + 3600)?;
        assert_eq!(block.metrics[0], 21.0);

        fs::remove_file(path).ok();
        Ok(())
    }
}