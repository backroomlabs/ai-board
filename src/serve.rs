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

/// Parse `design` out of a `/api/board?design=N` query string.
fn query_design(url: &str) -> Option<i64> {
    let q = url.split_once('?')?.1;
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "design")
        .and_then(|(_, v)| v.parse().ok())
}

fn parse_prefixed_id(path: &str, prefix: &str, label: &str) -> Result<i64> {
    let raw = path.trim_start_matches(prefix);
    if raw.is_empty() || raw.contains('/') {
        anyhow::bail!("invalid {label} id");
    }
    raw.parse()
        .map_err(|_| anyhow::anyhow!("invalid {label} id"))
}

/// Build the JSON body for a request path, or an error.
fn route_json(url: &str) -> Result<serde_json::Value> {
    let conn = db::open()?;
    let path = url.split('?').next().unwrap_or(url);
    match path {
        "/api/designs" => commands::designs_json(&conn),
        "/api/board" => {
            let id = query_design(url).ok_or_else(|| anyhow::anyhow!("missing ?design=<id>"))?;
            commands::board_json(&conn, id)
        }
        p if p.starts_with("/api/ticket/") => {
            let id = parse_prefixed_id(p, "/api/ticket/", "ticket")?;
            commands::ticket_json(&conn, id)
        }
        p if p.starts_with("/api/design/") => {
            let id: i64 = p
                .trim_start_matches("/api/design/")
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid design id"))?;
            commands::design_md_json(&conn, id)
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
    let spec = match parsed["spec"].as_str() {
        Some(v) => v,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: spec"}).to_string(),
                400,
            );
        }
    };
    let acceptance_criteria = match parsed.get("acceptance_criteria") {
        Some(v) => v,
        None => {
            return (
                serde_json::json!({"ok": false, "error": "missing field: acceptance_criteria"})
                    .to_string(),
                400,
            );
        }
    };

    match commands::update_ticket_content(id, title, spec, acceptance_criteria) {
        Ok(v) => (v.to_string(), 200),
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.contains("not found") { 404 } else { 400 };
            (serde_json::json!({"ok": false, "error": msg}).to_string(), code)
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

        if method == Method::Patch && url.starts_with("/api/ticket/") {
            let id = match parse_prefixed_id(&url, "/api/ticket/", "ticket") {
                Ok(v) => v,
                Err(e) => {
                    let body = serde_json::json!({"ok": false, "error": e.to_string()}).to_string();
                    let _ = request.respond(
                        Response::from_string(body).with_header(json_header()).with_status_code(400),
                    );
                    continue;
                }
            };
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                let body = serde_json::json!({"ok": false, "error": "failed to read request body"}).to_string();
                let _ = request.respond(
                    Response::from_string(body).with_header(json_header()).with_status_code(400),
                );
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let body = serde_json::json!({"ok": false, "error": e.to_string()}).to_string();
                    let _ = request.respond(
                        Response::from_string(body).with_header(json_header()).with_status_code(400),
                    );
                    continue;
                }
            };
            let (body, code) = patch_ticket_response(id, &parsed);
            let _ = request.respond(
                Response::from_string(body).with_header(json_header()).with_status_code(code),
            );
            continue;
        }

        if method == Method::Patch && url.starts_with("/api/design/") {
            let id_str = url.trim_start_matches("/api/design/");
            let id: i64 = match id_str.parse() {
                Ok(v) => v,
                Err(_) => {
                    let body = serde_json::json!({"ok": false, "error": "invalid design id"}).to_string();
                    let _ = request.respond(
                        Response::from_string(body).with_header(json_header()).with_status_code(400),
                    );
                    continue;
                }
            };
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                let body = serde_json::json!({"ok": false, "error": "failed to read request body"}).to_string();
                let _ = request.respond(
                    Response::from_string(body).with_header(json_header()).with_status_code(400),
                );
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let body = serde_json::json!({"ok": false, "error": e.to_string()}).to_string();
                    let _ = request.respond(
                        Response::from_string(body).with_header(json_header()).with_status_code(400),
                    );
                    continue;
                }
            };
            let title = match parsed["title"].as_str() {
                Some(v) => v,
                None => {
                    let body = serde_json::json!({"ok": false, "error": "missing field: title"}).to_string();
                    let _ = request.respond(
                        Response::from_string(body).with_header(json_header()).with_status_code(400),
                    );
                    continue;
                }
            };
            let design_md = match parsed["design_md"].as_str() {
                Some(v) => v,
                None => {
                    let body = serde_json::json!({"ok": false, "error": "missing field: design_md"}).to_string();
                    let _ = request.respond(
                        Response::from_string(body).with_header(json_header()).with_status_code(400),
                    );
                    continue;
                }
            };
            let (body, code) = match commands::update_design(id, title, design_md) {
                Ok(v) => (v.to_string(), 200),
                Err(e) => {
                    let msg = e.to_string();
                    let code = if msg.contains("not found") { 404 } else { 400 };
                    (serde_json::json!({"ok": false, "error": msg}).to_string(), code)
                }
            };
            let _ = request.respond(
                Response::from_string(body).with_header(json_header()).with_status_code(code),
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
                Err(e) => (
                    serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
                    if e.to_string().contains("not found") {
                        404
                    } else {
                        400
                    },
                ),
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
            "INSERT INTO design (title, design_md, status) VALUES ('D', 'design body', 'planning')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ticket (design_id, title, spec, acceptance_criteria, status) \
             VALUES (1, 'T', 'do work', '[\"cargo test => PASS\"]', 'queued')",
            [],
        )
        .unwrap();
        dir
    }

    #[test]
    fn route_json_returns_ticket_json() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let v = route_json("/api/ticket/1").unwrap();
        assert_eq!(v["id"], 1);
        assert_eq!(v["title"], "T");
        assert_eq!(v["design_md"], "design body");
    }

    #[test]
    fn route_json_rejects_invalid_ticket_id() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let err = route_json("/api/ticket/nope").unwrap_err().to_string();
        assert_eq!(err, "invalid ticket id");
    }

    #[test]
    fn patch_ticket_response_updates_ticket() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "spec": "updated spec",
            "acceptance_criteria": []
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 200);
        assert_eq!(v["title"], "Updated");
        assert_eq!(v["spec"], "updated spec");
        assert_eq!(v["acceptance_criteria"], serde_json::json!([]));
    }

    #[test]
    fn patch_ticket_response_rejects_missing_title() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "spec": "updated spec",
            "acceptance_criteria": []
        });

        let (body, code) = patch_ticket_response(1, &payload);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 400);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "missing field: title");
    }

    #[test]
    fn patch_ticket_response_rejects_unknown_ticket() {
        let _guard = env_lock().lock().unwrap();
        let _dir = with_temp_db();
        let payload = serde_json::json!({
            "title": "Updated",
            "spec": "updated spec",
            "acceptance_criteria": []
        });

        let (body, code) = patch_ticket_response(999, &payload);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(code, 404);
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "ticket 999 not found");
    }
}
