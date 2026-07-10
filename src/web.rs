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
            "/main.js" | "/script.js" => {
                let js = include_str!("web_static/main.js");
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
            "/api/delete-table" => {
                let mut content = String::new();
                let _ = request
                    .as_reader()
                    .read_to_string(&mut content);
                
                let table_name = content.trim();
                let response = match engine.write().delete_table(table_name) {
                    Ok(()) => {
                        let json = serde_json::json!({"result": format!("TABLE {} DROPPED", table_name)});
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
        include_str!("web_static/index.html").to_string()
    }
}

impl EngineStorage {
    /// Execute SQL (for web UI)
    pub fn execute_sql(&mut self, sql: &str) -> Result<String> {
        execute_sql(self, sql)
    }
}