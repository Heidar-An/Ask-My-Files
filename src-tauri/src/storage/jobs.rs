use crate::{models::IndexStatus, storage::open_connection, utils::unix_timestamp};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn create_index_job(conn: &Connection, root_id: i64, now: i64) -> Result<i64> {
    conn.execute(
        "UPDATE indexed_roots
         SET status = 'indexing',
             sync_status = 'syncing',
             last_error = NULL,
             updated_at = ?2
         WHERE id = ?1",
        params![root_id, now],
    )?;

    conn.execute(
        "INSERT INTO index_jobs (root_id, phase, status, processed, total, started_at)
         VALUES (?1, 'scan', 'running', 0, 0, ?2)",
        params![root_id, now],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn fetch_latest_jobs(conn: &Connection) -> Result<Vec<IndexStatus>> {
    let mut stmt = conn.prepare(
        "SELECT j.id,
                j.root_id,
                j.phase,
                j.status,
                j.processed,
                j.total,
                j.current_path,
                j.error_message,
                j.started_at,
                j.finished_at
         FROM index_jobs j
         INNER JOIN (
             SELECT root_id, MAX(id) AS latest_id
             FROM index_jobs
             GROUP BY root_id
         ) latest ON latest.latest_id = j.id
         ORDER BY j.started_at DESC",
    )?;

    let rows = stmt.query_map([], map_job_row)?;
    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row?);
    }
    Ok(jobs)
}

pub fn fetch_latest_job(conn: &Connection, root_id: i64) -> Result<Option<IndexStatus>> {
    let mut stmt = conn.prepare(
        "SELECT id,
                root_id,
                phase,
                status,
                processed,
                total,
                current_path,
                error_message,
                started_at,
                finished_at
         FROM index_jobs
         WHERE root_id = ?1
         ORDER BY id DESC
         LIMIT 1",
    )?;

    stmt.query_row(params![root_id], map_job_row)
        .optional()
        .map_err(Into::into)
}

pub fn fetch_job_by_id(conn: &Connection, job_id: i64) -> Result<Option<IndexStatus>> {
    let mut stmt = conn.prepare(
        "SELECT id,
                root_id,
                phase,
                status,
                processed,
                total,
                current_path,
                error_message,
                started_at,
                finished_at
         FROM index_jobs
         WHERE id = ?1",
    )?;

    stmt.query_row(params![job_id], map_job_row)
        .optional()
        .map_err(Into::into)
}

pub fn mark_job_failed(db_path: &Path, root_id: i64, job_id: i64, message: &str) -> Result<()> {
    let conn = open_connection(db_path)?;
    let now = unix_timestamp();
    conn.execute(
        "UPDATE indexed_roots
         SET status = 'error', sync_status = 'error', last_error = ?2, updated_at = ?3
         WHERE id = ?1",
        params![root_id, message, now],
    )?;
    conn.execute(
        "UPDATE index_jobs
         SET status = 'failed',
             phase = 'error',
             error_message = ?2,
             finished_at = ?3
         WHERE id = ?1",
        params![job_id, message, now],
    )?;
    Ok(())
}

pub fn update_job_progress(
    db_path: &Path,
    root_id: i64,
    job_id: i64,
    processed: u64,
    total: u64,
    current_path: Option<String>,
) -> Result<()> {
    let conn = open_connection(db_path)?;
    conn.execute(
        "UPDATE index_jobs
         SET processed = ?2,
             total = ?3,
             current_path = ?4
         WHERE id = ?1",
        params![job_id, processed as i64, total as i64, current_path],
    )?;
    conn.execute(
        "UPDATE indexed_roots
         SET file_count = ?2, updated_at = ?3
         WHERE id = ?1",
        params![root_id, processed as i64, unix_timestamp()],
    )?;
    Ok(())
}

pub fn update_root_ready(
    conn: &Connection,
    root_id: i64,
    job_id: i64,
    processed: u64,
    total: u64,
    last_error: Option<String>,
) -> Result<()> {
    let finished_at = unix_timestamp();
    conn.execute(
        "UPDATE indexed_roots
         SET status = 'ready',
             sync_status = 'watching',
             file_count = ?2,
             last_indexed_at = ?3,
             last_synced_at = ?3,
             last_error = ?4,
             updated_at = ?3
         WHERE id = ?1",
        params![root_id, processed as i64, finished_at, last_error],
    )?;
    conn.execute(
        "UPDATE index_jobs
         SET status = 'completed',
             processed = ?2,
             total = ?3,
             phase = 'complete',
             finished_at = ?4
         WHERE id = ?1",
        params![job_id, processed as i64, total as i64, finished_at],
    )?;
    Ok(())
}

fn map_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexStatus> {
    let error_message: Option<String> = row.get(7)?;
    Ok(IndexStatus {
        job_id: row.get(0)?,
        root_id: row.get(1)?,
        phase: row.get(2)?,
        status: row.get(3)?,
        processed: row.get::<_, i64>(4)? as u64,
        total: row.get::<_, i64>(5)? as u64,
        current_path: row.get(6)?,
        errors: error_message.into_iter().collect(),
        started_at: row.get(8)?,
        finished_at: row.get(9)?,
    })
}
