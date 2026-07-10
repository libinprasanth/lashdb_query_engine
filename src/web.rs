use crate::{execute_sql, EngineStorage, Result};
use parking_lot::RwLock;
use std::sync::Arc;
use tiny_http::{Server, Response, Request};

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
            "/style.css" => {
                let css = include_str!("web_static/style.css");
                let response = Response::from_string(css)
                    .with_status_code(200)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "text/css; charset=utf-8".as_bytes()).unwrap());
                let _ = request.respond(response);
            }
            "/script.js" => {
                let js = include_str!("web_static/script.js");
                let response = Response::from_string(js)
                    .with_status_code(200)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "application/javascript; charset=utf-8".as_bytes()).unwrap());
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
    <link rel="stylesheet" href="/style.css">
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
    
    <script src="/script.js"></script>
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