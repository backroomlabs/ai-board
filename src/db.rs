use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

/// Resolve the board DB path: `$BOARD_DB` or `./board.db`.
pub fn db_path() -> PathBuf {
    std::env::var_os("BOARD_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("board.db"))
}

pub fn preflight() -> Result<()> {
    let path = db_path();
    let conn = Connection::open(&path)?;
    reject_legacy_schema(&conn, &path)
}

pub fn open() -> Result<Connection> {
    let path = db_path();
    let conn = Connection::open(&path)?;
    reject_legacy_schema(&conn, &path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

fn reject_legacy_schema(conn: &Connection, path: &std::path::Path) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'design'
        )",
        [],
        |row| row.get(0),
    )?;
    if exists {
        anyhow::bail!(
            "legacy design schema detected at {}; recreate the board database",
            path.display()
        );
    }
    let ticket_exists: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'ticket'
        )",
        [],
        |row| row.get(0),
    )?;
    if ticket_exists {
        let has_old: bool = conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('ticket')
                WHERE name = 'acceptance_criteria'
            )",
            [],
            |row| row.get(0),
        )?;
        if has_old {
            anyhow::bail!(
                "legacy ticket.acceptance_criteria schema detected at {}; recreate the board database",
                path.display()
            );
        }
    }
    Ok(())
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS spec (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            title      TEXT NOT NULL,
            content    TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'planning',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS ticket (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            spec_id             INTEGER NOT NULL REFERENCES spec(id),
            title               TEXT NOT NULL,
            description         TEXT NOT NULL,
            definitions_of_done TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'queued',
            attempts            INTEGER NOT NULL DEFAULT 0,
            human_context       TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_ticket_spec ON ticket(spec_id);
        CREATE INDEX IF NOT EXISTS idx_ticket_status ON ticket(status);
        CREATE TABLE IF NOT EXISTS task (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_id           INTEGER NOT NULL REFERENCES ticket(id),
            title               TEXT NOT NULL,
            work_type           TEXT NOT NULL,
            objective           TEXT NOT NULL,
            acceptance_criteria TEXT NOT NULL,
            context             TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_task_ticket ON task(ticket_id);
        "#,
    )?;
    Ok(())
}
