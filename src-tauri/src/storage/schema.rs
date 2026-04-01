use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{path::Path, time::Duration};

pub fn initialize_database(path: &Path) -> Result<()> {
    let conn = open_connection(path)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS indexed_roots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'idle',
            file_count INTEGER NOT NULL DEFAULT 0,
            last_indexed_at INTEGER,
            last_error TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            root_id INTEGER NOT NULL REFERENCES indexed_roots(id) ON DELETE CASCADE,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            extension TEXT NOT NULL,
            kind TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER,
            indexed_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS index_jobs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            root_id INTEGER NOT NULL REFERENCES indexed_roots(id) ON DELETE CASCADE,
            phase TEXT NOT NULL,
            status TEXT NOT NULL,
            processed INTEGER NOT NULL DEFAULT 0,
            total INTEGER NOT NULL DEFAULT 0,
            current_path TEXT,
            error_message TEXT,
            started_at INTEGER NOT NULL,
            finished_at INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_files_root_id ON files(root_id);
        CREATE INDEX IF NOT EXISTS idx_files_name ON files(name);
        CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
        CREATE INDEX IF NOT EXISTS idx_files_modified_at ON files(modified_at DESC);
        CREATE INDEX IF NOT EXISTS idx_index_jobs_root_id ON index_jobs(root_id, id DESC);
        ",
    )?;
    Ok(())
}

pub fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open database at {}", path.display()))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    Ok(conn)
}
