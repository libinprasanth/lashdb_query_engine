use crate::{execute_sql, EngineStorage, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

const WELCOME_MESSAGE: &str = "flashdb-query-engine TCP server\nAvailable commands: HELP, PING, GENERATE <hours>, READ_AT <timestamp>, SUM_AT <timestamp>, COUNT, LIST TABLES, DESCRIBE <table>, QUERY <SQL>\nSupported SQL: SELECT (with WHERE), CREATE TABLE, INSERT, CREATE USER\n";

pub fn run_tcp_server(path: impl AsRef<Path>, addr: &str) -> Result<()> {
    let mut engine = EngineStorage::open(path)?;
    let listener = TcpListener::bind(addr)?;

    println!("Server listening on {}", addr);
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if let Err(err) = handle_connection(&mut engine, stream) {
                    eprintln!("connection error: {}", err);
                }
            }
            Err(err) => {
                eprintln!("listener error: {}", err);
            }
        }
    }
    Ok(())
}

fn handle_connection(engine: &mut EngineStorage, stream: TcpStream) -> Result<()> {
    let peer = stream.peer_addr().ok();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // Send welcome message, handle client disconnect gracefully
    if writer.write_all(WELCOME_MESSAGE.as_bytes()).is_err() || writer.flush().is_err() {
        return Ok(());
    }

    let mut line = String::new();
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

        let response = match parse_command(engine, trimmed) {
            Ok(text) => text,
            Err(err) => format!("ERR {}\n", err),
        };

        // Handle client disconnect gracefully
        if writer.write_all(response.as_bytes()).is_err() || writer.flush().is_err() {
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

fn parse_command(engine: &mut EngineStorage, command_line: &str) -> Result<String> {
    let parts: Vec<&str> = command_line.split_whitespace().collect();
    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("HELP") => Ok(format!("{}\n", WELCOME_MESSAGE)),
        [cmd] if cmd.eq_ignore_ascii_case("PING") => Ok("PONG\n".to_string()),
        [cmd] if cmd.eq_ignore_ascii_case("LIST") => Ok(list_tables(engine)),
        [cmd, arg] if cmd.eq_ignore_ascii_case("DESCRIBE") => {
            describe_table(engine, arg)
        }
        [cmd, arg] if cmd.eq_ignore_ascii_case("GENERATE") => {
            let hours = arg.parse::<i64>().map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid hours"))?;
            engine.generate_mock_database(hours)?;
            Ok(format!("OK generated {} hours\n", hours))
        }
        [cmd, arg] if cmd.eq_ignore_ascii_case("READ_AT") => {
            let timestamp = arg.parse::<i64>().map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid timestamp"))?;
            let block = engine.read_block_at_time(timestamp)?;
            let metrics = block.metrics.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
            Ok(format!("BLOCK {}\n", metrics))
        }
        [cmd, arg] if cmd.eq_ignore_ascii_case("SUM_AT") => {
            let timestamp = arg.parse::<i64>().map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid timestamp"))?;
            let block = engine.read_block_at_time(timestamp)?;
            let sum: f32 = block.metrics.iter().copied().sum();
            Ok(format!("SUM {}\n", sum))
        }
        [cmd] if cmd.eq_ignore_ascii_case("COUNT") => {
            let count = engine.block_count()?;
            Ok(format!("COUNT {}\n", count))
        }
        _ if command_line.to_uppercase().starts_with("QUERY ") => {
            let sql = command_line[6..].trim();
            let result = execute_sql(engine, sql)?;
            Ok(format!("OK {}\n", result))
        }
        _ if command_line.to_uppercase().starts_with("SELECT ")
            || command_line.to_uppercase().starts_with("CREATE ")
            || command_line.to_uppercase().starts_with("INSERT ") =>
        {
            let result = execute_sql(engine, command_line)?;
            Ok(format!("OK {}\n", result))
        }
        [cmd] if cmd.eq_ignore_ascii_case("QUIT") => Ok("BYE\n".to_string()),
        [cmd] => Ok(format!("ERR unknown command: {}\n", cmd)),
        _ => Ok("ERR malformed command\n".to_string()),
    }
}

fn list_tables(engine: &mut EngineStorage) -> String {
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

fn describe_table(engine: &mut EngineStorage, table_name: &str) -> Result<String> {
    let schema = engine.get_table_schema(table_name)?;
    let columns = schema
        .columns
        .iter()
        .map(|c| format!("  {} {}", c.name, c.data_type))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("TABLE {}\n{}\n", schema.name, columns))
}