use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

/// Resolve the board DB path: `$BOARD_DB` or `./board.db`.
pub fn db_path() -> PathBuf {
    std::env::var_os("BOARD_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("board.db"))
}

pub fn open() -> Result<Connection> {
    let conn = Connection::open(db_path())?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS design (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            title      TEXT NOT NULL,
            design_md  TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'planning',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS ticket (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            design_id           INTEGER NOT NULL REFERENCES design(id),
            title               TEXT NOT NULL,
            spec                TEXT NOT NULL,
            acceptance_criteria TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'queued',
            attempts            INTEGER NOT NULL DEFAULT 0,
            human_context       TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_ticket_design ON ticket(design_id);
        CREATE INDEX IF NOT EXISTS idx_ticket_status ON ticket(status);
        "#,
    )?;
    Ok(())
}
