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
            acceptance_criteria TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'queued',
            attempts            INTEGER NOT NULL DEFAULT 0,
            human_context       TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_ticket_spec ON ticket(spec_id);
        CREATE INDEX IF NOT EXISTS idx_ticket_status ON ticket(status);
        "#,
    )?;
    Ok(())
}
