use crate::{execute_sql, EngineStorage, Result};
use parking_lot::RwLock;
use std::sync::Arc;
use tiny_http::{Server, Response, Request};
use std::io::Read;

/// Simple web UI for database management (phpMyAdmin-like)
pub struct WebUI {
    engine: Arc<RwLock<EngineStorage>>,
    port: u16,
}

impl WebUI {
    /// Create a new web UI server
    pub fn new(engine: Arc<RwLock<EngineStorage>>, port: u16) -> Self {
        Self { engine, port }
    }

    /// Start the web server
    pub fn run(&self) -> Result<()> {
        let server = Server::http(format!("0.0.0.0:{}", self.port))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        println!("Web UI available at http://localhost:{}/", self.port);

        for request in server.incoming_requests() {
            let engine = Arc::clone(&self.engine);
            self.handle_request(request, &engine);
        }

        Ok(())
    }

    fn handle_request(&self, mut request: Request, engine: &Arc<RwLock<EngineStorage>>) {
        let url = request.url().to_string();
        
        match url.as_str() {
            "/" | "/index.html" => {
                let html = self.render_index();
                let response = Response::from_string(html)
                    .with_status_code(200)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8".as_bytes()).unwrap());
                let _ = request.respond(response);
            }
            "/api/tables" => {
                let response = match engine.read().load_catalog() {
                    Ok(catalog) => {
                        let tables: Vec<&str> = catalog.tables.iter().map(|t| t.name.as_str()).collect();
                        Response::from_string(serde_json::to_string(&tables).unwrap())
                            .with_status_code(200)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                    }
                    Err(e) => {
                        let json = serde_json::json!({"error": e.to_string()});
                        Response::from_string(json.to_string())
                            .with_status_code(500)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                    }
                };
                let _ = request.respond(response);
            }
            "/api/query" => {
                let mut content = String::new();
                let _ = request
                    .as_reader()
                    .read_to_string(&mut content);
                
                let sql = content.trim();
                let response = match engine.write().execute_sql(sql) {
                    Ok(result) => {
                        let json = serde_json::json!({"result": result});
                        Response::from_string(json.to_string())
                            .with_status_code(200)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                    }
                    Err(e) => {
                        let json = serde_json::json!({"error": e.to_string()});
                        Response::from_string(json.to_string())
                            .with_status_code(400)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                    }
                };
                let _ = request.respond(response);
            }
            _ => {
                let response = Response::from_string("Not Found")
                    .with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }

    fn render_index(&self) -> String {
        r#"<!DOCTYPE html>
<html>
<head>
    <title>FlashDB Query Engine</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        h1 { color: #333; border-bottom: 2px solid #4CAF50; padding-bottom: 10px; }
        .panel { margin: 20px 0; padding: 15px; border: 1px solid #ddd; border-radius: 4px; }
        .panel h2 { margin-top: 0; color: #4CAF50; }
        textarea, input { width: 100%; padding: 10px; margin: 5px 0; border: 1px solid #ccc; border-radius: 4px; }
        button { background: #4CAF50; color: white; padding: 10px 20px; border: none; border-radius: 4px; cursor: pointer; }
        button:hover { background: #45a049; }
        .result { background: #f9f9f9; padding: 10px; margin-top: 10px; border-radius: 4px; white-space: pre-wrap; }
        .tables { display: flex; flex-wrap: wrap; gap: 10px; }
        .table-card { background: #e8f5e9; padding: 15px; border-radius: 4px; cursor: pointer; min-width: 150px; }
        .table-card:hover { background: #c8e6c9; }
    </style>
</head>
<body>
    <div class="container">
        <h1>FlashDB Query Engine</h1>
        
        <div class="panel">
            <h2>Tables</h2>
            <div id="tables" class="tables">Loading...</div>
        </div>
        
        <div class="panel">
            <h2>SQL Query</h2>
            <textarea id="sql" rows="4" placeholder="SELECT * FROM products">SELECT * FROM products</textarea>
            <button onclick="executeQuery()">Execute</button>
            <div id="result" class="result"></div>
        </div>
    </div>
    
    <script>
        async function loadTables() {
            try {
                const response = await fetch('/api/tables');
                const tables = await response.json();
                const html = tables.map(t => `<div class="table-card" onclick="selectTable('${t}')">${t}</div>`).join('');
                document.getElementById('tables').innerHTML = html || 'No tables found';
            } catch (e) {
                document.getElementById('tables').innerHTML = 'Error loading tables';
            }
        }
        
        async function executeQuery() {
            const sql = document.getElementById('sql').value;
            try {
                const response = await fetch('/api/query', {
                    method: 'POST',
                    body: sql
                });
                const data = await response.json();
                document.getElementById('result').textContent = data.result || data.error;
            } catch (e) {
                document.getElementById('result').textContent = 'Error: ' + e;
            }
        }
        
        function selectTable(table) {
            document.getElementById('sql').value = `SELECT * FROM ${table}`;
        }
        
        loadTables();
    </script>
</body>
</html>"#.to_string()
    }
}

impl EngineStorage {
    /// Execute SQL (for web UI)
    pub fn execute_sql(&mut self, sql: &str) -> Result<String> {
        execute_sql(self, sql)
    }
}