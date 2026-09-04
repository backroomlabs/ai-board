use anyhow::Result;
use tiny_http::{Header, Method, Response, Server};

use crate::{commands, db};

const INDEX_HTML: &str = include_str!("ui/index.html");

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
}

fn html_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap()
}

fn query_spec_id(url: &str) -> Option<i64> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == "spec_id")
        .and_then(|(_, value)| value.parse().ok())
}

fn query_ticket_id(url: &str) -> Option<i64> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == "ticket_id")
        .and_then(|(_, value)| value.parse().ok())
}

fn request_path(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

fn parse_prefixed_id(path: &str, prefix: &str, label: &str) -> Result<i64> {
    let raw = path.trim_start_matches(prefix);
    if raw.is_empty() || raw.contains('/') {
        anyhow::bail!("invalid {label} id");
    }
    raw.parse()
        .map_err(|_| anyhow::anyhow!("invalid {label} id"))
}

fn parse_json_body(raw: &str) -> std::result::Result<serde_json::Value, String> {
    serde_json::from_str(raw).map_err(|e| e.to_string())
}

fn api_error_status(message: &str) -> u16 {
    if message.contains("not found") {
        404
    } else {
        400
    }
}

/// Build the JSON body for a request path, or an error.
fn route_json(url: &str) -> Result<serde_json::Value> {
    let conn = db::open()?;
    let path = request_path(url);
    match path {
        "/api/specs" => commands::specs_json(&conn),
        "/api/board" => {
            let spec_id =
                query_spec_id(url).ok_or_else(|| anyhow::anyhow!("missing ?spec_id=<id>"))?;
            commands::board_json(&conn, spec_id)
        }
        "/api/tasks" => {
            let ticket_id = query_ticket_id(url)
                .ok_or_else(|| anyhow::anyhow!("missing ?ticket_id=<id>"))?;
            commands::list_tasks_json(&conn, ticket_id)
        }
        path if path.starts_with("/api/task/") => {
            let id = parse_prefixed_id(path, "/api/task/", "task")?;
            commands::task_json(&conn, id)
        }
        path if path.starts_with("/api/ticket/") => {
            let id = parse_prefixed_id(path, "/api/ticket/", "ticket")?;
            commands::ticket_json(&conn, id)
        }
        path if path.starts_with("/api/spec/") => {
            let id = parse_prefixed_id(path, "/api/spec/", "spec")?;
            commands::spec_json(&conn, id)
        }
        _ => anyhow::bail!("not found"),
    }
}

fn port_in_use(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn patch_ticket_response(id: i64, parsed: &serde_json::Value) -> (String, u16) {
    let title = match parsed["title"].as_str() {
        Some(v) => v,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: title"}).to_string(),
                400,
            );
        }
    };
    let description = match parsed["description"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({
                    "ok": false,
                    "error": "missing field: description"
                })
                .to_string(),
                400,
            );
        }
    };
    let definitions_of_done = match parsed.get("definitions_of_done") {
        Some(v) => v,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: definitions_of_done"})
                    .to_string(),
                400,
            );
        }
    };

    match commands::update_ticket_content(id, title, description, definitions_of_done) {
        Ok(v) => (v.to_string(), 200),
        Err(e) => {
            let msg = e.to_string();
            let code = api_error_status(&msg);
            (
                serde_json::json!({"ok": false, "error": msg}).to_string(),
                code,
            )
        }
    }
}

fn patch_spec_response(id: i64, parsed: &serde_json::Value) -> (String, u16) {
    let title = match parsed["title"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: title"}).to_string(),
                400,
            );
        }
    };
    let content = match parsed["content"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: content"}).to_string(),
                400,
            );
        }
    };

    match commands::update_spec(id, title, content) {
        Ok(value) => (value.to_string(), 200),
        Err(error) => {
            let message = error.to_string();
            let code = api_error_status(&message);
            (
                serde_json::json!({"ok": false, "error": message}).to_string(),
                code,
            )
        }
    }
}

fn post_task_response(parsed: &serde_json::Value) -> (String, u16) {
    let ticket_id = match parsed["ticket_id"].as_i64() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: ticket_id"}).to_string(),
                400,
            );
        }
    };
    let title = match parsed["title"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: title"}).to_string(),
                400,
            );
        }
    };
    let work_type = match parsed["work_type"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: work_type"}).to_string(),
                400,
            );
        }
    };
    let objective = match parsed["objective"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: objective"}).to_string(),
                400,
            );
        }
    };
    let acceptance_criteria = match parsed.get("acceptance_criteria") {
        Some(value) if value.is_array() => value,
        Some(_) => {
            return (
                serde_json::json!({
                    "ok": false,
                    "error": "acceptance_criteria must be a JSON array"
                })
                .to_string(),
                400,
            );
        }
        None => {
            return (
                serde_json::json!({
                    "ok": false,
                    "error": "missing field: acceptance_criteria"
                })
                .to_string(),
                400,
            );
        }
    };
    let context = parsed["context"].as_str();
    let criteria = match serde_json::to_string(acceptance_criteria) {
        Ok(value) => value,
        Err(error) => {
            return (
                serde_json::json!({"ok": false, "error": error.to_string()}).to_string(),
                400,
            );
        }
    };

    match commands::add_task(ticket_id, title, work_type, objective, &criteria, context) {
        Ok(value) => (value.to_string(), 200),
        Err(error) => {
            let message = error.to_string();
            let code = api_error_status(&message);
            (
                serde_json::json!({"ok": false, "error": message}).to_string(),
                code,
            )
        }
    }
}

fn patch_task_response(id: i64, parsed: &serde_json::Value) -> (String, u16) {
    let title = match parsed["title"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: title"}).to_string(),
                400,
            );
        }
    };
    let work_type = match parsed["work_type"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: work_type"}).to_string(),
                400,
            );
        }
    };
    let objective = match parsed["objective"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: objective"}).to_string(),
                400,
            );
        }
    };
    let acceptance_criteria = match parsed.get("acceptance_criteria") {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({
                    "ok": false,
                    "error": "missing field: acceptance_criteria"
                })
                .to_string(),
                400,
            );
        }
    };
    let context = match parsed["context"].as_str() {
        Some(value) => value,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: context"}).to_string(),
                400,
            );
        }
    };

    match commands::update_task_content(id, title, work_type, objective, acceptance_criteria, context)
    {
        Ok(value) => (value.to_string(), 200),
        Err(error) => {
            let message = error.to_string();
            let code = api_error_status(&message);
            (
                serde_json::json!({"ok": false, "error": message}).to_string(),
                code,
            )
        }
    }
}

pub fn serve(port: u16) -> Result<()> {
    if port_in_use(port) {
        eprintln!("abd serve: already running on port {port}");
        return Ok(());
    }
    let addr = format!("0.0.0.0:{port}");
    let server = Server::http(&addr).map_err(|e| anyhow::anyhow!("bind {addr}: {e}"))?;
    eprintln!("abd serve: http://{addr}  (Ctrl-C to stop)");

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();
        let path = request_path(&url);

        if method == Method::Post && path == "/api/tasks" {
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                let body = serde_json::json!({"ok": false, "error": "failed to read request body"})
                    .to_string();
                let _ = request.respond(
                    Response::from_string(body)
                        .with_header(json_header())
                        .with_status_code(400),
                );
                continue;
            }
            let parsed = match parse_json_body(&raw) {
                Ok(value) => value,
                Err(error) => {
                    let body = serde_json::json!({"ok": false, "error": error}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let (body, code) = post_task_response(&parsed);
            let _ = request.respond(
                Response::from_string(body)
                    .with_header(json_header())
                    .with_status_code(code),
            );
            continue;
        }

        if method == Method::Patch && path.starts_with("/api/task/") {
            let id = match parse_prefixed_id(path, "/api/task/", "task") {
                Ok(id) => id,
                Err(error) => {
                    let body =
                        serde_json::json!({"ok": false, "error": error.to_string()}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                let body = serde_json::json!({"ok": false, "error": "failed to read request body"})
                    .to_string();
                let _ = request.respond(
                    Response::from_string(body)
                        .with_header(json_header())
                        .with_status_code(400),
                );
                continue;
            }
            let parsed = match parse_json_body(&raw) {
                Ok(value) => value,
                Err(error) => {
                    let body = serde_json::json!({"ok": false, "error": error}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let (body, code) = patch_task_response(id, &parsed);
            let _ = request.respond(
                Response::from_string(body)
                    .with_header(json_header())
                    .with_status_code(code),
            );
            continue;
        }

        if method == Method::Patch && path.starts_with("/api/ticket/") {
            let id = match parse_prefixed_id(path, "/api/ticket/", "ticket") {
                Ok(v) => v,
                Err(e) => {
                    let body = serde_json::json!({"ok": false, "error": e.to_string()}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                let body = serde_json::json!({"ok": false, "error": "failed to read request body"})
                    .to_string();
                let _ = request.respond(
                    Response::from_string(body)
                        .with_header(json_header())
                        .with_status_code(400),
                );
                continue;
            }
            let parsed: serde_json::Value = match parse_json_body(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let body = serde_json::json!({"ok": false, "error": e}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let (body, code) = patch_ticket_response(id, &parsed);
            let _ = request.respond(
                Response::from_string(body)
                    .with_header(json_header())
                    .with_status_code(code),
            );
            continue;
        }

        if method == Method::Patch && path.starts_with("/api/spec/") {
            let id = match parse_prefixed_id(path, "/api/spec/", "spec") {
                Ok(id) => id,
                Err(error) => {
                    let body =
                        serde_json::json!({"ok": false, "error": error.to_string()}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                let body = serde_json::json!({"ok": false, "error": "failed to read request body"})
                    .to_string();
                let _ = request.respond(
                    Response::from_string(body)
                        .with_header(json_header())
                        .with_status_code(400),
                );
                continue;
            }
            let parsed = match parse_json_body(&raw) {
                Ok(value) => value,
                Err(error) => {
                    let body = serde_json::json!({"ok": false, "error": error}).to_string();
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(json_header())
                            .with_status_code(400),
                    );
                    continue;
                }
            };
            let (body, code) = patch_spec_response(id, &parsed);
            let _ = request.respond(
                Response::from_string(body)
                    .with_header(json_header())
                    .with_status_code(code),
            );
            continue;
        }

        if method != Method::Get {
            let _ =
                request.respond(Response::from_string("method not allowed").with_status_code(405));
            continue;
        }

        if url == "/" || url.starts_with("/?") {
            let resp = Response::from_string(INDEX_HTML).with_header(html_header());
            let _ = request.respond(resp);
            continue;
        }

        if url.starts_with("/api/") {
            let (body, code) = match route_json(&url) {
                Ok(v) => (v.to_string(), 200),
                Err(e) => {
                    let msg = e.to_string();
                    (
                        serde_json::json!({"ok": false, "error": msg}).to_string(),
                        api_error_status(&msg),
                    )
                }
            };
            let resp = Response::from_string(body)
                .with_header(json_header())
                .with_status_code(code);
            let _ = request.respond(resp);
            continue;
        }

        let _ = request.respond(Response::from_string("not found").with_status_code(404));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    #[test]
    fn embedded_ui_uses_spec_routes_and_description() {
        assert!(INDEX_HTML.contains("fetch(\"/api/specs\")"));
        assert!(INDEX_HTML.contains("/api/board?spec_id=${id}"));
        assert!(INDEX_HTML.contains("/api/spec/${specId}"));
        assert!(INDEX_HTML.contains("ticket.description"));
        assert!(INDEX_HTML.contains("const description = editorRef.current"));
        assert!(INDEX_HTML.contains("description,"));
        assert!(!INDEX_HTML.contains("/api/design"));
        assert!(!INDEX_HTML.contains("ticket.spec"));
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_db() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::env::set_var("BOARD_DB", dir.path().join("board.db"));
        let conn = db::open().unwrap();
        db::init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO spec (title, content, status)
             VALUES ('S', 'spec content', 'planning')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ticket
             (spec_id, title, description, definitions_of_done, status)
             VALUES (1, 'T', 'do work', '[\"cargo test => PASS\"]', 'queued')",
            [],
        )
        .unwrap();
        dir
    }

    #[test]
    fn route_json_returns_ticket_without_parent_content() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let value = route_json("/api/ticket/1").unwrap();
        assert_eq!(value["spec_id"], 1);
        assert_eq!(value["description"], "do work");
        assert!(value.get("content").is_none());
    }

    #[test]
    fn route_json_returns_specs_and_selected_board() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();

        let specs = route_json("/api/specs").unwrap();
        assert_eq!(specs[0]["title"], "S");

        let board = route_json("/api/board?spec_id=1").unwrap();
        assert_eq!(board["spec"]["id"], 1);
        assert_eq!(board["tickets"][0]["description"], "do work");
    }

    #[test]
    fn route_json_returns_structured_spec() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let value = route_json("/api/spec/1").unwrap();
        assert_eq!(value["title"], "S");
        assert_eq!(value["content"], "spec content");
    }

    #[test]
    fn old_design_routes_are_not_found() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        assert!(route_json("/api/designs").is_err());
        assert!(route_json("/api/design/1").is_err());
        assert!(route_json("/api/board?design=1").is_err());
    }

    #[test]
    fn route_json_rejects_invalid_ticket_id() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let err = route_json("/api/ticket/nope").unwrap_err().to_string();
        assert_eq!(err, "invalid ticket id");
    }

    #[test]
    fn parse_json_body_rejects_malformed_json() {
        let err = parse_json_body("{").unwrap_err();
        assert!(err.contains("EOF") || err.contains("expected"));
    }

    #[test]
    fn request_path_strips_query_string() {
        assert_eq!(request_path("/api/ticket/1?foo=bar"), "/api/ticket/1");
    }

    #[test]
    fn patch_ticket_response_updates_ticket() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "description": "updated description",
            "definitions_of_done": []
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 200);
        assert_eq!(value["title"], "Updated");
        assert_eq!(value["description"], "updated description");
        assert_eq!(value["definitions_of_done"], serde_json::json!([]));
    }

    #[test]
    fn patch_ticket_response_rejects_empty_description() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Valid title",
            "description": "   ",
            "definitions_of_done": []
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 400);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], "description must not be empty");
    }

    #[test]
    fn patch_ticket_response_rejects_non_array_dod() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Valid title",
            "description": "Valid description",
            "definitions_of_done": {"bad": true}
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 400);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "definitions_of_done must be a JSON array");
    }

    #[test]
    fn patch_ticket_response_rejects_missing_title() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "description": "updated description",
            "definitions_of_done": []
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 400);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "missing field: title");
    }

    #[test]
    fn patch_ticket_response_rejects_missing_description() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "definitions_of_done": []
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 400);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], "missing field: description");
    }

    #[test]
    fn patch_ticket_response_rejects_unknown_ticket() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "description": "updated description",
            "definitions_of_done": []
        });

        let (body, code) = patch_ticket_response(999, &payload);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 404);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "ticket 999 not found");
    }

    #[test]
    fn route_json_unknown_ticket_maps_to_404_status() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let msg = route_json("/api/ticket/999").unwrap_err().to_string();
        assert_eq!(api_error_status(&msg), 404);
    }

    #[test]
    fn patch_spec_response_updates_title_and_content() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "content": "# Updated"
        });

        let (body, code) = patch_spec_response(1, &payload);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(code, 200);
        assert_eq!(value["title"], "Updated");
        assert_eq!(value["content"], "# Updated");
    }

    #[test]
    fn route_json_ticket_includes_tasks_board_does_not() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let conn = db::open().unwrap();
        conn.execute(
            "INSERT INTO task (ticket_id, title, work_type, objective, acceptance_criteria, context)
             VALUES (1, 'range', 'code_implementation', 'detect', '[]', '')",
            [],
        )
        .unwrap();

        let ticket = route_json("/api/ticket/1").unwrap();
        assert_eq!(ticket["tasks"][0]["title"], "range");
        assert!(ticket.get("acceptance_criteria").is_none());
        assert!(ticket["definitions_of_done"].is_array());

        let board = route_json("/api/board?spec_id=1").unwrap();
        assert!(board["tickets"][0].get("tasks").is_none());

        let tasks = route_json("/api/tasks?ticket_id=1").unwrap();
        assert_eq!(tasks.as_array().unwrap().len(), 1);

        let task = route_json("/api/task/1").unwrap();
        assert_eq!(task["title"], "range");
    }

    #[test]
    fn post_and_patch_task() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let (body, code) = post_task_response(&serde_json::json!({
            "ticket_id": 1,
            "title": "range",
            "work_type": "code_implementation",
            "objective": "detect",
            "acceptance_criteria": ["cargo test => PASS"]
        }));
        assert_eq!(code, 200);
        let created: serde_json::Value = serde_json::from_str(&body).unwrap();
        let id = created["id"].as_i64().unwrap();

        let (body, code) = patch_task_response(id, &serde_json::json!({
            "title": "range 2",
            "work_type": "investigation",
            "objective": "survey",
            "acceptance_criteria": [],
            "context": "see src/range.rs"
        }));
        assert_eq!(code, 200);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["title"], "range 2");
        assert_eq!(value["work_type"], "investigation");
        assert_eq!(value["context"], "see src/range.rs");
    }

    #[test]
    fn post_task_rejects_unknown_work_type() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let (body, code) = post_task_response(&serde_json::json!({
            "ticket_id": 1,
            "title": "x",
            "work_type": "refactor",
            "objective": "x",
            "acceptance_criteria": []
        }));
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(code, 400);
        assert!(value["error"].as_str().unwrap().contains("invalid work type"));
    }

    #[test]
    fn patch_ticket_rejects_old_acceptance_criteria_key() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "description": "updated description",
            "acceptance_criteria": []
        });
        let (body, code) = patch_ticket_response(1, &payload);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(code, 400);
        assert_eq!(value["error"], "missing field: definitions_of_done");
    }
}
