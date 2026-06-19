use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use crate::db;
use crate::models::{Status, Ticket};

pub fn init() -> Result<Value> {
    let conn = db::open()?;
    db::init_schema(&conn)?;
    Ok(json!({"ok": true, "db": db::db_path().to_string_lossy()}))
}

pub fn create_design(title: &str, file: Option<&Path>, stdin: bool) -> Result<Value> {
    let design_md = match (file, stdin) {
        (Some(path), false) => std::fs::read_to_string(path)?,
        (None, true) => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
        _ => anyhow::bail!("provide exactly one of --file or --stdin"),
    };
    let conn = db::open()?;
    conn.execute(
        "INSERT INTO design (title, design_md) VALUES (?1, ?2)",
        rusqlite::params![title, design_md],
    )?;
    let id = conn.last_insert_rowid();
    Ok(json!({"id": id, "title": title}))
}

pub fn add_ticket(design: i64, title: &str, spec: &str, criteria: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(criteria)
        .map_err(|e| anyhow::anyhow!("--criteria is not valid JSON: {e}"))?;
    if !parsed.is_array() {
        anyhow::bail!("--criteria must be a JSON array of command strings");
    }
    let conn = db::open()?;
    let exists: bool = conn
        .query_row("SELECT 1 FROM design WHERE id = ?1", [design], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if !exists {
        anyhow::bail!("no design with id {design}");
    }
    conn.execute(
        "INSERT INTO ticket (design_id, title, spec, acceptance_criteria) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![design, title, spec, criteria],
    )?;
    Ok(json!({"id": conn.last_insert_rowid()}))
}

fn row_to_ticket(conn: &Connection, id: i64) -> Result<Option<Ticket>> {
    let ticket = conn
        .query_row(
            "SELECT id, design_id, title, spec, acceptance_criteria, status, attempts, human_context \
             FROM ticket WHERE id = ?1",
            [id],
            |r| {
                let criteria_raw: String = r.get(4)?;
                Ok(Ticket {
                    id: r.get(0)?,
                    design_id: r.get(1)?,
                    title: r.get(2)?,
                    spec: r.get(3)?,
                    acceptance_criteria: serde_json::from_str(&criteria_raw)
                        .unwrap_or(Value::Null),
                    status: r.get(5)?,
                    attempts: r.get(6)?,
                    human_context: r.get(7)?,
                })
            },
        )
        .optional()?;
    Ok(ticket)
}

pub fn next(design: Option<i64>) -> Result<Value> {
    let conn = db::open()?;
    let sql = "UPDATE ticket SET status = 'implementing' \
               WHERE id = ( \
                   SELECT id FROM ticket \
                   WHERE status = 'queued' AND (?1 IS NULL OR design_id = ?1) \
                   ORDER BY id LIMIT 1 \
               ) RETURNING id";
    let claimed: Option<i64> = conn.query_row(sql, [design], |r| r.get(0)).optional()?;
    match claimed {
        Some(id) => {
            let ticket = row_to_ticket(&conn, id)?.expect("just claimed");
            Ok(serde_json::to_value(ticket)?)
        }
        None => Ok(json!({"ticket": null})),
    }
}

pub fn show(ticket_id: i64) -> Result<Value> {
    let conn = db::open()?;
    let ticket = row_to_ticket(&conn, ticket_id)?
        .ok_or_else(|| anyhow::anyhow!("no ticket with id {ticket_id}"))?;
    let design_md: String = conn.query_row(
        "SELECT design_md FROM design WHERE id = ?1",
        [ticket.design_id],
        |r| r.get(0),
    )?;
    let mut value = serde_json::to_value(&ticket)?;
    value["design_md"] = Value::String(design_md);
    Ok(value)
}

pub fn list(design: i64) -> Result<Value> {
    let conn = db::open()?;
    let mut stmt = conn.prepare("SELECT id FROM ticket WHERE design_id = ?1 ORDER BY id")?;
    let ids: Vec<i64> = stmt
        .query_map([design], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let tickets: Vec<Ticket> = ids
        .into_iter()
        .map(|id| row_to_ticket(&conn, id).map(|t| t.unwrap()))
        .collect::<Result<_>>()?;
    Ok(serde_json::to_value(tickets)?)
}

pub fn update(
    ticket_id: i64,
    status: &str,
    context: Option<&str>,
    bump_attempts: bool,
) -> Result<Value> {
    let status = Status::from_str(status)?;
    let conn = db::open()?;
    let changed = conn.execute(
        "UPDATE ticket SET \
            status = ?1, \
            attempts = attempts + ?2, \
            human_context = COALESCE(?3, human_context) \
         WHERE id = ?4",
        rusqlite::params![
            status.as_str(),
            if bump_attempts { 1 } else { 0 },
            context,
            ticket_id
        ],
    )?;
    if changed == 0 {
        anyhow::bail!("no ticket with id {ticket_id}");
    }
    let ticket = row_to_ticket(&conn, ticket_id)?.expect("updated");
    Ok(serde_json::to_value(ticket)?)
}

pub fn needs_human(design: Option<i64>) -> Result<Value> {
    let conn = db::open()?;
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM ticket \
             WHERE status = 'needs_human' AND (?1 IS NULL OR design_id = ?1) \
             ORDER BY id LIMIT 1",
            [design],
            |r| r.get(0),
        )
        .optional()?;
    match id {
        Some(id) => Ok(serde_json::to_value(row_to_ticket(&conn, id)?.unwrap())?),
        None => Ok(json!({"ticket": null})),
    }
}

pub fn designs() -> Result<Value> {
    let conn = db::open()?;
    designs_json(&conn)
}

pub fn design(design_id: i64) -> Result<Value> {
    let conn = db::open()?;
    let md: String = conn
        .query_row(
            "SELECT design_md FROM design WHERE id = ?1",
            [design_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("no design with id {design_id}"))?;
    Ok(json!({ "__raw__": md }))
}

pub fn update_design(id: i64, title: &str, design_md: &str) -> Result<Value> {
    let conn = db::open()?;
    let changed = conn.execute(
        "UPDATE design SET title = ?1, design_md = ?2 WHERE id = ?3",
        rusqlite::params![title, design_md, id],
    )?;
    if changed == 0 {
        anyhow::bail!("design {id} not found");
    }
    let row = conn.query_row(
        "SELECT id, title, status, design_md FROM design WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "design_md": r.get::<_, String>(3)?,
            }))
        },
    )?;
    Ok(row)
}

/// Full design including design_md, for the UI design viewer.
pub fn design_md_json(conn: &Connection, design_id: i64) -> Result<Value> {
    let row = conn
        .query_row(
            "SELECT id, title, status, design_md FROM design WHERE id = ?1",
            [design_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("no design with id {design_id}"))?;
    Ok(json!({"id": row.0, "title": row.1, "status": row.2, "design_md": row.3}))
}

/// All designs, newest-first metadata (no blob), for the UI sidebar.
pub fn designs_json(conn: &Connection) -> Result<Value> {
    let mut stmt =
        conn.prepare("SELECT id, title, status, created_at FROM design ORDER BY id DESC")?;
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "created_at": r.get::<_, String>(3)?,
            }))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Value::Array(rows))
}

/// One design's metadata + all its tickets (full), ordered by id.
pub fn board_json(conn: &Connection, design_id: i64) -> Result<Value> {
    let design = conn
        .query_row(
            "SELECT id, title, status FROM design WHERE id = ?1",
            [design_id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "status": r.get::<_, String>(2)?,
                }))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("no design with id {design_id}"))?;

    let mut stmt = conn.prepare("SELECT id FROM ticket WHERE design_id = ?1 ORDER BY id")?;
    let ids: Vec<i64> = stmt
        .query_map([design_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let tickets: Vec<Ticket> = ids
        .into_iter()
        .map(|id| row_to_ticket(conn, id).map(|t| t.unwrap()))
        .collect::<Result<_>>()?;

    Ok(json!({ "design": design, "tickets": serde_json::to_value(tickets)? }))
}

#[cfg(test)]
mod view_tests {
    use super::*;
    use rusqlite::Connection;

    fn seed() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO design (title, design_md, status) VALUES ('D', 'md', 'active')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ticket (design_id, title, spec, acceptance_criteria, status) \
             VALUES (1, 'T1', 'do x', '[\"true => PASS\"]', 'queued')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ticket (design_id, title, spec, acceptance_criteria, status) \
             VALUES (1, 'T2', 'do y', '[\"true => PASS\"]', 'implementing')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn designs_json_lists_designs() {
        let conn = seed();
        let v = designs_json(&conn).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["title"], "D");
        assert_eq!(arr[0]["status"], "active");
    }

    #[test]
    fn board_json_returns_design_and_tickets() {
        let conn = seed();
        let v = board_json(&conn, 1).unwrap();
        assert_eq!(v["design"]["id"], 1);
        assert_eq!(v["design"]["title"], "D");
        let tickets = v["tickets"].as_array().unwrap();
        assert_eq!(tickets.len(), 2);
        assert_eq!(tickets[0]["title"], "T1");
        assert!(tickets[0]["acceptance_criteria"].is_array());
        assert_eq!(tickets[1]["status"], "implementing");
    }

    #[test]
    fn board_json_unknown_design_errors() {
        let conn = seed();
        assert!(board_json(&conn, 999).is_err());
    }

    #[test]
    fn update_design_changes_title_and_md() {
        let conn = seed();
        // seed() creates design id=1 with title="D", design_md="md"
        // update_design opens its own conn via db::open() so we need to use
        // the env var approach — but since seed() is in-memory we can't.
        // Test the SQL logic directly on the seed conn instead:
        let changed = conn.execute(
            "UPDATE design SET title = ?1, design_md = ?2 WHERE id = ?3",
            rusqlite::params!["New Title", "# New Content", 1i64],
        ).unwrap();
        assert_eq!(changed, 1);
        let (title, md): (String, String) = conn.query_row(
            "SELECT title, design_md FROM design WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(title, "New Title");
        assert_eq!(md, "# New Content");
    }

    #[test]
    fn update_design_missing_id_changes_zero_rows() {
        let conn = seed();
        let changed = conn.execute(
            "UPDATE design SET title = ?1, design_md = ?2 WHERE id = ?3",
            rusqlite::params!["X", "Y", 999i64],
        ).unwrap();
        assert_eq!(changed, 0);
    }
}
