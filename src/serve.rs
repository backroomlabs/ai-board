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

pub fn serve(port: u16) -> Result<()> {
    if port_in_use(port) {
        eprintln!("board serve: already running on port {port}");
        return Ok(());
    }
    let addr = format!("0.0.0.0:{port}");
    let server = Server::http(&addr).map_err(|e| anyhow::anyhow!("bind {addr}: {e}"))?;
    eprintln!("board serve: http://{addr}  (Ctrl-C to stop)");

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let is_get = *request.method() == Method::Get;

        if !is_get {
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
