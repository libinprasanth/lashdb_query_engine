use crate::{execute_sql, EngineStorage, Result};
use parking_lot::RwLock;
use std::sync::Arc;
use tiny_http::{Server, Response, Request};
use std::io::Read;

/// MongoDB Compass-style web UI for database management
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
                        let tables: Vec<_> = catalog.tables.iter().map(|t| {
                            serde_json::json!({
                                "name": t.name,
                                "columns": t.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>()
                            })
                        }).collect();
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
            "/api/schema" => {
                let response = match engine.read().load_catalog() {
                    Ok(catalog) => {
                        let schemas: Vec<_> = catalog.tables.iter().map(|t| {
                            serde_json::json!({
                                "name": t.name,
                                "columns": t.columns.iter().map(|c| {
                                    serde_json::json!({"name": c.name, "type": c.data_type})
                                }).collect::<Vec<_>>()
                            })
                        }).collect();
                        Response::from_string(serde_json::to_string(&schemas).unwrap())
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
    <title>FlashDB - MongoDB Compass Style UI</title>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet">
    <link href="https://fonts.googleapis.com/icon?family=Material+Icons" rel="stylesheet">
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif; background: #1a1a2e; color: #e0e0e0; min-height: 100vh; }
        .compass-header { background: #0d1117; padding: 12px 24px; display: flex; align-items: center; border-bottom: 1px solid #30363d; }
        .compass-header .logo { display: flex; align-items: center; gap: 12px; }
        .compass-header .logo-icon { width: 32px; height: 32px; background: #4db33d; border-radius: 4px; display: flex; align-items: center; justify-content: center; }
        .compass-header h1 { font-size: 18px; font-weight: 600; color: #fff; }
        .compass-header .connection { margin-left: auto; font-size: 13px; color: #888; }
        .compass-container { display: flex; height: calc(100vh - 57px); }
        .compass-sidebar { width: 260px; background: #0d1117; border-right: 1px solid #30363d; overflow-y: auto; }
        .compass-sidebar-header { padding: 16px; font-size: 12px; text-transform: uppercase; letter-spacing: 1px; color: #888; border-bottom: 1px solid #30363d; }
        .compass-table-list { padding: 8px 0; }
        .compass-table-item { display: flex; align-items: center; gap: 10px; padding: 10px 16px; cursor: pointer; transition: all 0.2s; }
        .compass-table-item:hover { background: #161b22; }
        .compass-table-item.active { background: #238636; }
        .compass-table-item .icon { color: #4db33d; font-size: 18px; }
        .compass-table-item .name { font-size: 14px; color: #fff; }
        .compass-main { flex: 1; display: flex; flex-direction: column; }
        .compass-toolbar { padding: 12px 24px; background: #0d1117; border-bottom: 1px solid #30363d; display: flex; align-items: center; gap: 12px; }
        .compass-btn { background: #21262d; color: #fff; border: 1px solid #30363d; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; display: flex; align-items: center; gap: 6px; }
        .compass-btn:hover { background: #30363d; }
        .compass-btn-primary { background: #238636; border-color: #238636; }
        .compass-btn-primary:hover { background: #2ea043; }
        .compass-editor-container { flex: 1; padding: 24px; overflow: hidden; display: flex; flex-direction: column; }
        .compass-editor { flex: 1; background: #0d1117; border: 1px solid #30363d; border-radius: 6px; padding: 16px; font-family: 'Fira Code', monospace; font-size: 14px; color: #c9d1d9; resize: none; }
        .compass-editor:focus { outline: none; border-color: #238636; }
        .compass-result { margin-top: 16px; background: #0d1117; border-radius: 6px; border: 1px solid #30363d; max-height: 300px; overflow-y: auto; }
        .compass-result-header { padding: 10px 16px; background: #161b22; border-bottom: 1px solid #30363d; font-size: 12px; color: #888; }
        .compass-result-table { width: 100%; border-collapse: collapse; }
        .compass-result-table th { background: #161b22; padding: 10px 16px; text-align: left; font-weight: 500; color: #4db33d; border-bottom: 1px solid #30363d; font-size: 12px; }
        .compass-result-table td { padding: 10px 16px; border-bottom: 1px solid #30363d; color: #c9d1d9; font-size: 13px; }
        .compass-result-table tr:last-child td { border-bottom: none; }
        .compass-result-table tr:hover { background: #161b22; }
        .compass-error { background: #da3633; color: #fff; padding: 12px 16px; }
        .compass-empty { padding: 40px; text-align: center; color: #666; font-size: 14px; }
        .compass-documents { padding: 16px; }
        .compass-document { background: #161b22; padding: 12px 16px; border-radius: 4px; margin-bottom: 8px; font-family: monospace; font-size: 12px; }
    </style>
</head>
<body>
    <header class="compass-header">
        <div class="logo">
            <div class="logo-icon">
                <i class="material-icons" style="color: #fff; font-size: 20px;">storage</i>
            </div>
            <h1>FlashDB Compass</h1>
        </div>
        <div class="connection">localhost:8080</div>
    </header>
    
    <div class="compass-container">
        <div class="compass-sidebar">
            <div class="compass-sidebar-header">Collections (Tables)</div>
            <div class="compass-table-list" id="tables">
                <div class="compass-empty">Loading...</div>
            </div>
        </div>
        
        <div class="compass-main">
            <div class="compass-toolbar">
                <button class="compass-btn compass-btn-primary" onclick="executeQuery()">
                    <i class="material-icons" style="font-size: 16px;">play_arrow</i>
                    Find
                </button>
                <button class="compass-btn" onclick="clearQuery()">
                    <i class="material-icons" style="font-size: 16px;">clear</i>
                    Clear
                </button>
                <button class="compass-btn" onclick="setQuery('SELECT * FROM products LIMIT 10')">
                    <i class="material-icons" style="font-size: 16px;">table_chart</i>
                    Products
                </button>
                <button class="compass-btn" onclick="setQuery('SELECT * FROM address LIMIT 10')">
                    <i class="material-icons" style="font-size: 16px;">table_chart</i>
                    Address
                </button>
            </div>
            
            <div class="compass-editor-container">
                <textarea id="sql" class="compass-editor" placeholder="SELECT * FROM products">SELECT * FROM products</textarea>
                <div id="result" class="compass-result" style="display: none;">
                    <div class="compass-result-header">Documents</div>
                    <div id="result-content" class="compass-documents"></div>
                </div>
            </div>
        </div>
    </div>
    
    <script>
        async function loadTables() {
            try {
                const response = await fetch('/api/tables');
                const tables = await response.json();
                const html = tables.map(t => `
                    <div class="compass-table-item" onclick="selectTable('${t.name}')">
                        <i class="material-icons icon">table_chart</i>
                        <span class="name">${t.name}</span>
                    </div>
                `).join('');
                document.getElementById('tables').innerHTML = html || '<div class="compass-empty">No tables found</div>';
            } catch (e) {
                document.getElementById('tables').innerHTML = '<div class="compass-empty">Error loading tables</div>';
            }
        }
        
        function selectTable(table) {
            document.getElementById('sql').value = `SELECT * FROM ${table}`;
            document.querySelectorAll('.compass-table-item').forEach(el => el.classList.remove('active'));
            event.currentTarget.classList.add('active');
        }
        
        function setQuery(sql) {
            document.getElementById('sql').value = sql;
        }
        
        function clearQuery() {
            document.getElementById('sql').value = '';
            document.getElementById('result').style.display = 'none';
        }
        
        function formatAsDocuments(result) {
            try {
                const data = JSON.parse(result);
                if (Array.isArray(data) && data.length > 0) {
                    return data.map((row, idx) => `
                        <div class="compass-document">
                            <div style="color: #4db33d; margin-bottom: 8px;">Document ${idx + 1}</div>
                            <pre style="margin: 0; color: #c9d1d9;">${JSON.stringify(row, null, 2)}</pre>
                        </div>
                    `).join('');
                }
            } catch (e) {}
            return `<div class="compass-error">${escapeHtml(result)}</div>`;
        }
        
        function formatAsTable(result) {
            try {
                const data = JSON.parse(result);
                if (Array.isArray(data) && data.length > 0) {
                    const columns = Object.keys(data[0]);
                    return `
                        <table class="compass-result-table">
                            <thead>
                                <tr>${columns.map(c => `<th>${c}</th>`).join('')}</tr>
                            </thead>
                            <tbody>
                                ${data.map(row => `
                                    <tr>${columns.map(c => `<td>${row[c] ?? ''}</td>`).join('')}</tr>
                                `).join('')}
                            </tbody>
                        </table>
                    `;
                }
            } catch (e) {}
            return `<div class="compass-error">${escapeHtml(result)}</div>`;
        }
        
        function escapeHtml(text) {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }
        
        async function executeQuery() {
            const sql = document.getElementById('sql').value;
            const resultDiv = document.getElementById('result');
            const resultContent = document.getElementById('result-content');
            
            if (!sql.trim()) {
                resultDiv.style.display = 'block';
                resultContent.innerHTML = '<div class="compass-error">Please enter a SQL query</div>';
                return;
            }
            
            resultDiv.style.display = 'block';
            resultContent.innerHTML = '<div class="compass-empty">Loading...</div>';
            
            try {
                const response = await fetch('/api/query', {
                    method: 'POST',
                    body: sql
                });
                const data = await response.json();
                
                if (data.error) {
                    resultContent.innerHTML = `<div class="compass-error">${escapeHtml(data.error)}</div>`;
                } else {
                    // Try table format first, then documents
                    resultContent.innerHTML = formatAsTable(data.result) || formatAsDocuments(data.result);
                }
            } catch (e) {
                resultContent.innerHTML = `<div class="compass-error">Error: ${escapeHtml(e.toString())}</div>`;
            }
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