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
            "/assets/index.js" => {
                let js = include_str!("web_static/dist/assets/index.js");
                let response = Response::from_string(js)
                    .with_status_code(200)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "application/javascript; charset=utf-8".as_bytes()).unwrap());
                let _ = request.respond(response);
            }
            "/assets/index.css" => {
                let css = include_str!("web_static/dist/assets/index.css");
                let response = Response::from_string(css)
                    .with_status_code(200)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "text/css; charset=utf-8".as_bytes()).unwrap());
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
            "/api/users" => {
                let response = match engine.read().load_catalog() {
                    Ok(catalog) => {
                        let users: Vec<_> = catalog.users.iter().map(|u| {
                            serde_json::json!({
                                "username": u.username,
                                "hasPassword": u.password.is_some()
                            })
                        }).collect();
                        Response::from_string(serde_json::to_string(&users).unwrap())
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
            "/api/create-user" => {
                let mut content = String::new();
                let _ = request
                    .as_reader()
                    .read_to_string(&mut content);
                
                let user_data: serde_json::Value = match serde_json::from_str(&content) {
                    Ok(data) => data,
                    Err(e) => {
                        let json = serde_json::json!({"error": format!("Invalid JSON: {}", e)});
                        let response = Response::from_string(json.to_string())
                            .with_status_code(400)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap());
                        let _ = request.respond(response);
                        return;
                    }
                };
                
                let username = user_data.get("username").and_then(|v| v.as_str()).unwrap_or("");
                let password = user_data.get("password").and_then(|v| v.as_str());
                
                let response = match engine.write().create_user(username, password.map(|s| s.to_string())) {
                    Ok(()) => {
                        let json = serde_json::json!({"result": format!("User {} created", username)});
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
            "/api/authenticate" => {
                let mut content = String::new();
                let _ = request
                    .as_reader()
                    .read_to_string(&mut content);
                
                let auth_data: serde_json::Value = match serde_json::from_str(&content) {
                    Ok(data) => data,
                    Err(e) => {
                        let json = serde_json::json!({"error": format!("Invalid JSON: {}", e)});
                        let response = Response::from_string(json.to_string())
                            .with_status_code(400)
                            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap());
                        let _ = request.respond(response);
                        return;
                    }
                };
                
                let username = auth_data.get("username").and_then(|v| v.as_str()).unwrap_or("");
                let password = auth_data.get("password").and_then(|v| v.as_str()).unwrap_or("");
                
                let response = match engine.read().load_catalog() {
                    Ok(catalog) => {
                        let user = catalog.users.iter().find(|u| u.username.eq_ignore_ascii_case(username));
                        if let Some(u) = user {
                            // Check password if user has one, or allow if no password set
                            if u.password.is_none() || u.password.as_ref().map(|p| p == password).unwrap_or(false) {
                                let json = serde_json::json!({"authenticated": true, "username": username});
                                Response::from_string(json.to_string())
                                    .with_status_code(200)
                                    .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                            } else {
                                let json = serde_json::json!({"authenticated": false, "error": "Invalid password"});
                                Response::from_string(json.to_string())
                                    .with_status_code(401)
                                    .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                            }
                        } else {
                            let json = serde_json::json!({"authenticated": false, "error": "User not found"});
                            Response::from_string(json.to_string())
                                .with_status_code(401)
                                .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json".as_bytes()).unwrap())
                        }
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
            _ => {
                let response = Response::from_string("Not Found")
                    .with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }

    fn render_index(&self) -> String {
        include_str!("web_static/dist/index.html").to_string()
    }
}

impl EngineStorage {
    /// Execute SQL (for web UI)
    pub fn execute_sql(&mut self, sql: &str) -> Result<String> {
        execute_sql(self, sql)
    }
}