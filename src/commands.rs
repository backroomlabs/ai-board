use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use crate::db;
use crate::models::{Status, Task, Ticket, WorkType};

fn ensure_spec_exists(conn: &Connection, spec_id: i64) -> Result<()> {
    let exists = conn
        .query_row("SELECT 1 FROM spec WHERE id = ?1", [spec_id], |_| Ok(()))
        .optional()?;
    if exists.is_none() {
        anyhow::bail!("spec {spec_id} not found");
    }
    Ok(())
}

fn ensure_ticket_exists(conn: &Connection, ticket_id: i64) -> Result<()> {
    let exists = conn
        .query_row("SELECT 1 FROM ticket WHERE id = ?1", [ticket_id], |_| Ok(()))
        .optional()?;
    if exists.is_none() {
        anyhow::bail!("ticket {ticket_id} not found");
    }
    Ok(())
}

fn parse_json_array(raw: &str, flag: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|error| anyhow::anyhow!("{flag} is not valid JSON: {error}"))?;
    if !parsed.is_array() {
        anyhow::bail!("{flag} must be a JSON array");
    }
    Ok(parsed)
}

pub fn init() -> Result<Value> {
    let conn = db::open()?;
    db::init_schema(&conn)?;
    Ok(json!({"ok": true, "db": db::db_path().to_string_lossy()}))
}

pub fn add_spec(title: &str, file: Option<&Path>, stdin: bool) -> Result<Value> {
    let content = match (file, stdin) {
        (Some(path), false) => std::fs::read_to_string(path)?,
        (None, true) => {
            let mut buffer = String::new();
            std::io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
        _ => anyhow::bail!("provide exactly one of --file or --stdin"),
    };
    let conn = db::open()?;
    conn.execute(
        "INSERT INTO spec (title, content) VALUES (?1, ?2)",
        rusqlite::params![title, content],
    )?;
    Ok(json!({"id": conn.last_insert_rowid(), "title": title}))
}

pub fn add_ticket(spec_id: i64, title: &str, description: &str, dod: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(dod)
        .map_err(|error| anyhow::anyhow!("--dod is not valid JSON: {error}"))?;
    if !parsed.is_array() {
        anyhow::bail!("--dod must be a JSON array");
    }
    let conn = db::open()?;
    ensure_spec_exists(&conn, spec_id)?;
    conn.execute(
        "INSERT INTO ticket (spec_id, title, description, definitions_of_done)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![spec_id, title, description, dod],
    )?;
    Ok(json!({"id": conn.last_insert_rowid()}))
}

fn row_to_ticket(conn: &Connection, id: i64) -> Result<Option<Ticket>> {
    let ticket = conn
        .query_row(
            "SELECT id, spec_id, title, description, definitions_of_done,
                    status, attempts, human_context
             FROM ticket WHERE id = ?1",
            [id],
            |row| {
                let dod_raw: String = row.get(4)?;
                Ok(Ticket {
                    id: row.get(0)?,
                    spec_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    definitions_of_done: serde_json::from_str(&dod_raw).unwrap_or(Value::Null),
                    status: row.get(5)?,
                    attempts: row.get(6)?,
                    human_context: row.get(7)?,
                })
            },
        )
        .optional()?;
    Ok(ticket)
}

pub fn next(spec_id: Option<i64>) -> Result<Value> {
    let conn = db::open()?;
    if let Some(spec_id) = spec_id {
        ensure_spec_exists(&conn, spec_id)?;
    }
    let sql = "UPDATE ticket SET status = 'implementing' \
               WHERE id = ( \
                   SELECT id FROM ticket \
                   WHERE status = 'queued' AND (?1 IS NULL OR spec_id = ?1) \
                   ORDER BY id LIMIT 1 \
               ) RETURNING id";
    let claimed: Option<i64> = conn
        .query_row(sql, [spec_id], |row| row.get(0))
        .optional()?;
    match claimed {
        Some(id) => {
            let ticket = row_to_ticket(&conn, id)?.expect("just claimed");
            Ok(serde_json::to_value(ticket)?)
        }
        None => Ok(json!({"ticket": null})),
    }
}

fn row_to_task(conn: &Connection, id: i64) -> Result<Option<Task>> {
    let task = conn
        .query_row(
            "SELECT id, ticket_id, title, work_type, objective, acceptance_criteria, context
             FROM task WHERE id = ?1",
            [id],
            |row| {
                let criteria_raw: String = row.get(5)?;
                Ok(Task {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    title: row.get(2)?,
                    work_type: row.get(3)?,
                    objective: row.get(4)?,
                    acceptance_criteria: serde_json::from_str(&criteria_raw).unwrap_or(Value::Null),
                    context: row.get(6)?,
                })
            },
        )
        .optional()?;
    Ok(task)
}

fn tasks_for_ticket(conn: &Connection, ticket_id: i64) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM task WHERE ticket_id = ?1 ORDER BY id",
    )?;
    let ids: Vec<i64> = stmt
        .query_map([ticket_id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    ids.into_iter()
        .map(|id| row_to_task(conn, id).map(|t| t.expect("just selected")))
        .collect()
}

pub fn add_task(
    ticket_id: i64,
    title: &str,
    work_type: &str,
    objective: &str,
    criteria: &str,
    context: Option<&str>,
) -> Result<Value> {
    if title.trim().is_empty() {
        anyhow::bail!("title must not be empty");
    }
    if objective.trim().is_empty() {
        anyhow::bail!("objective must not be empty");
    }
    let work_type = WorkType::from_str(work_type)?;
    let _ = parse_json_array(criteria, "--criteria")?;
    let conn = db::open()?;
    ensure_ticket_exists(&conn, ticket_id)?;
    conn.execute(
        "INSERT INTO task (ticket_id, title, work_type, objective, acceptance_criteria, context)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            ticket_id,
            title,
            work_type.as_str(),
            objective,
            criteria,
            context.unwrap_or("")
        ],
    )?;
    Ok(json!({"id": conn.last_insert_rowid()}))
}

pub fn list_tasks(ticket_id: i64) -> Result<Value> {
    let conn = db::open()?;
    ensure_ticket_exists(&conn, ticket_id)?;
    Ok(serde_json::to_value(tasks_for_ticket(&conn, ticket_id)?)?)
}

pub fn show_task(task_id: i64) -> Result<Value> {
    let conn = db::open()?;
    task_json(&conn, task_id)
}

pub fn task_json(conn: &Connection, task_id: i64) -> Result<Value> {
    let task = row_to_task(conn, task_id)?
        .ok_or_else(|| anyhow::anyhow!("task {task_id} not found"))?;
    Ok(serde_json::to_value(task)?)
}

pub fn ticket_json(conn: &Connection, ticket_id: i64) -> Result<Value> {
    let ticket = row_to_ticket(conn, ticket_id)?
        .ok_or_else(|| anyhow::anyhow!("ticket {ticket_id} not found"))?;
    let mut value = serde_json::to_value(ticket)?;
    value["tasks"] = serde_json::to_value(tasks_for_ticket(conn, ticket_id)?)?;
    Ok(value)
}

pub fn show(ticket_id: i64) -> Result<Value> {
    let conn = db::open()?;
    ticket_json(&conn, ticket_id)
}

pub fn update_ticket_content_json(
    conn: &Connection,
    ticket_id: i64,
    title: &str,
    description: &str,
    definitions_of_done: &Value,
) -> Result<Value> {
    if title.trim().is_empty() {
        anyhow::bail!("title must not be empty");
    }
    if description.trim().is_empty() {
        anyhow::bail!("description must not be empty");
    }
    if !definitions_of_done.is_array() {
        anyhow::bail!("definitions_of_done must be a JSON array");
    }

    let dod_json = serde_json::to_string(definitions_of_done)?;
    let changed = conn.execute(
        "UPDATE ticket
         SET title = ?1, description = ?2, definitions_of_done = ?3
         WHERE id = ?4",
        rusqlite::params![title, description, dod_json, ticket_id],
    )?;
    if changed == 0 {
        anyhow::bail!("ticket {ticket_id} not found");
    }
    ticket_json(conn, ticket_id)
}

pub fn update_ticket_content(
    ticket_id: i64,
    title: &str,
    description: &str,
    definitions_of_done: &Value,
) -> Result<Value> {
    let conn = db::open()?;
    update_ticket_content_json(&conn, ticket_id, title, description, definitions_of_done)
}

pub fn list(spec_id: i64) -> Result<Value> {
    let conn = db::open()?;
    ensure_spec_exists(&conn, spec_id)?;
    let mut stmt = conn.prepare("SELECT id FROM ticket WHERE spec_id = ?1 ORDER BY id")?;
    let ids: Vec<i64> = stmt
        .query_map([spec_id], |row| row.get(0))?
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

pub fn needs_human(spec_id: Option<i64>) -> Result<Value> {
    let conn = db::open()?;
    if let Some(spec_id) = spec_id {
        ensure_spec_exists(&conn, spec_id)?;
    }
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM ticket \
             WHERE status = 'needs_human' AND (?1 IS NULL OR spec_id = ?1) \
             ORDER BY id LIMIT 1",
            [spec_id],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => Ok(serde_json::to_value(row_to_ticket(&conn, id)?.unwrap())?),
        None => Ok(json!({"ticket": null})),
    }
}

pub fn specs() -> Result<Value> {
    let conn = db::open()?;
    specs_json(&conn)
}

pub fn get_spec(spec_id: i64) -> Result<Value> {
    let conn = db::open()?;
    let content: String = conn
        .query_row("SELECT content FROM spec WHERE id = ?1", [spec_id], |row| {
            row.get(0)
        })
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("spec {spec_id} not found"))?;
    Ok(json!({"__raw__": content}))
}

pub fn update_spec(id: i64, title: &str, content: &str) -> Result<Value> {
    let conn = db::open()?;
    let changed = conn.execute(
        "UPDATE spec SET title = ?1, content = ?2 WHERE id = ?3",
        rusqlite::params![title, content, id],
    )?;
    if changed == 0 {
        anyhow::bail!("spec {id} not found");
    }
    let row = conn.query_row(
        "SELECT id, title, status, content FROM spec WHERE id = ?1",
        [id],
        |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "content": row.get::<_, String>(3)?,
            }))
        },
    )?;
    Ok(row)
}

pub fn spec_json(conn: &Connection, spec_id: i64) -> Result<Value> {
    let row = conn
        .query_row(
            "SELECT id, title, status, content FROM spec WHERE id = ?1",
            [spec_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("spec {spec_id} not found"))?;
    Ok(json!({"id": row.0, "title": row.1, "status": row.2, "content": row.3}))
}

pub fn specs_json(conn: &Connection) -> Result<Value> {
    let mut stmt =
        conn.prepare("SELECT id, title, status, created_at FROM spec ORDER BY id DESC")?;
    let rows: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Value::Array(rows))
}

pub fn board_json(conn: &Connection, spec_id: i64) -> Result<Value> {
    let spec = conn
        .query_row(
            "SELECT id, title, status FROM spec WHERE id = ?1",
            [spec_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "status": row.get::<_, String>(2)?,
                }))
            },
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("spec {spec_id} not found"))?;

    let mut stmt = conn.prepare("SELECT id FROM ticket WHERE spec_id = ?1 ORDER BY id")?;
    let ids: Vec<i64> = stmt
        .query_map([spec_id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let tickets: Vec<Ticket> = ids
        .into_iter()
        .map(|id| row_to_ticket(conn, id).map(|t| t.unwrap()))
        .collect::<Result<_>>()?;

    Ok(json!({ "spec": spec, "tickets": serde_json::to_value(tickets)? }))
}

#[cfg(test)]
mod view_tests {
    use super::*;
    use rusqlite::Connection;

    fn seed() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO spec (title, content, status) VALUES ('S', 'content', 'active')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ticket (spec_id, title, description, definitions_of_done, status) \
             VALUES (1, 'T1', 'do x', '[\"true => PASS\"]', 'queued')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ticket (spec_id, title, description, definitions_of_done, status) \
             VALUES (1, 'T2', 'do y', '[\"true => PASS\"]', 'implementing')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn specs_json_lists_specs() {
        let conn = seed();
        let v = specs_json(&conn).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["title"], "S");
        assert_eq!(arr[0]["status"], "active");
    }

    #[test]
    fn board_json_returns_spec_and_tickets() {
        let conn = seed();
        let v = board_json(&conn, 1).unwrap();
        assert_eq!(v["spec"]["id"], 1);
        assert_eq!(v["spec"]["title"], "S");
        let tickets = v["tickets"].as_array().unwrap();
        assert_eq!(tickets.len(), 2);
        assert_eq!(tickets[0]["title"], "T1");
        assert!(tickets[0]["definitions_of_done"].is_array());
        assert!(tickets[0].get("tasks").is_none());
        assert_eq!(tickets[1]["status"], "implementing");
    }

    #[test]
    fn board_json_unknown_spec_errors() {
        let conn = seed();
        assert!(board_json(&conn, 999).is_err());
    }

    #[test]
    fn ticket_json_excludes_parent_content() {
        let conn = seed();
        let v = ticket_json(&conn, 1).unwrap();
        assert_eq!(v["id"], 1);
        assert_eq!(v["title"], "T1");
        assert_eq!(v["spec_id"], 1);
        assert_eq!(v["description"], "do x");
        assert!(v.get("content").is_none());
        assert!(v["definitions_of_done"].is_array());
    }

    #[test]
    fn update_ticket_content_changes_only_content_fields() {
        let conn = seed();
        conn.execute(
            "UPDATE ticket SET status = 'needs_human', attempts = 2, human_context = 'blocked' WHERE id = 1",
            [],
        )
        .unwrap();

        let dod = json!(["cargo test => PASS", "cargo fmt --check => PASS"]);
        let v = update_ticket_content_json(
            &conn,
            1,
            "New ticket title",
            "New ticket description",
            &dod,
        )
        .unwrap();

        assert_eq!(v["title"], "New ticket title");
        assert_eq!(v["description"], "New ticket description");
        assert_eq!(v["definitions_of_done"], dod);
        assert_eq!(v["status"], "needs_human");
        assert_eq!(v["attempts"], 2);
        assert_eq!(v["human_context"], "blocked");
    }

    #[test]
    fn update_ticket_content_allows_empty_criteria_array() {
        let conn = seed();
        let v = update_ticket_content_json(&conn, 1, "Title", "Description", &json!([])).unwrap();
        assert_eq!(v["definitions_of_done"], json!([]));
    }

    #[test]
    fn update_ticket_content_rejects_empty_title() {
        let conn = seed();
        let err = update_ticket_content_json(&conn, 1, "   ", "Description", &json!([]))
            .unwrap_err()
            .to_string();
        assert_eq!(err, "title must not be empty");
    }

    #[test]
    fn update_ticket_content_rejects_empty_description() {
        let conn = seed();
        let err = update_ticket_content_json(&conn, 1, "Title", "\n\t", &json!([]))
            .unwrap_err()
            .to_string();
        assert_eq!(err, "description must not be empty");
    }

    #[test]
    fn update_ticket_content_rejects_non_array_criteria() {
        let conn = seed();
        let err =
            update_ticket_content_json(&conn, 1, "Title", "Description", &json!({"bad": true}))
                .unwrap_err()
                .to_string();
        assert_eq!(err, "definitions_of_done must be a JSON array");
    }

    #[test]
    fn update_ticket_content_rejects_unknown_ticket() {
        let conn = seed();
        let err = update_ticket_content_json(&conn, 999, "Title", "Description", &json!([]))
            .unwrap_err()
            .to_string();
        assert_eq!(err, "ticket 999 not found");
    }

    #[test]
    fn update_spec_changes_title_and_content() {
        let conn = seed();
        let changed = conn
            .execute(
                "UPDATE spec SET title = ?1, content = ?2 WHERE id = ?3",
                rusqlite::params!["New Title", "# New Content", 1i64],
            )
            .unwrap();
        assert_eq!(changed, 1);
        let (title, content): (String, String) = conn
            .query_row("SELECT title, content FROM spec WHERE id = 1", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(title, "New Title");
        assert_eq!(content, "# New Content");
    }

    #[test]
    fn update_spec_missing_id_changes_zero_rows() {
        let conn = seed();
        let changed = conn
            .execute(
                "UPDATE spec SET title = ?1, content = ?2 WHERE id = ?3",
                rusqlite::params!["X", "Y", 999i64],
            )
            .unwrap();
        assert_eq!(changed, 0);
    }
}
