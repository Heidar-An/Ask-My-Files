use crate::models::IndexedRoot;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

const ROOT_SELECT: &str = "
    SELECT r.id,
           r.path,
           r.status,
           r.sync_status,
           r.file_count,
           COALESCE((
               SELECT COUNT(*)
               FROM file_extracts e
               JOIN files f ON f.id = e.file_id
               WHERE f.root_id = r.id AND e.status = 'indexed'
           ), 0) AS content_indexed_count,
           COALESCE((
               SELECT COUNT(*)
               FROM file_extracts e
               JOIN files f ON f.id = e.file_id
               WHERE f.root_id = r.id AND e.status = 'pending'
           ), 0) AS content_pending_count,
           COALESCE((
               SELECT COUNT(*)
               FROM file_semantic_index s
               JOIN files f ON f.id = s.file_id
               WHERE f.root_id = r.id AND s.status = 'indexed'
           ), 0) AS semantic_indexed_count,
           COALESCE((
               SELECT COUNT(*)
               FROM file_semantic_index s
               JOIN files f ON f.id = s.file_id
               WHERE f.root_id = r.id AND s.status = 'pending'
           ), 0) AS semantic_pending_count,
           r.last_indexed_at,
           r.last_synced_at,
           r.last_change_at,
           r.last_error
    FROM indexed_roots r
";

pub fn fetch_roots(conn: &Connection) -> Result<Vec<IndexedRoot>> {
    let mut stmt = conn.prepare(&format!("{ROOT_SELECT} ORDER BY r.path COLLATE NOCASE"))?;

    let rows = stmt.query_map([], map_root_row)?;
    let mut roots = Vec::new();
    for row in rows {
        roots.push(row?);
    }
    Ok(roots)
}

pub fn insert_or_update_root(conn: &Connection, path: &str, now: i64) -> Result<IndexedRoot> {
    conn.execute(
        "INSERT INTO indexed_roots (path, status, sync_status, file_count, created_at, updated_at)
         VALUES (?1, 'idle', 'watching', 0, ?2, ?2)
         ON CONFLICT(path) DO UPDATE SET
            sync_status = 'watching',
            updated_at = excluded.updated_at",
        params![path, now],
    )?;

    let root = conn.query_row(
        &format!("{ROOT_SELECT} WHERE r.path = ?1"),
        params![path],
        map_root_row,
    )?;

    Ok(root)
}

pub fn remove_root(conn: &Connection, root_id: i64) -> Result<()> {
    conn.execute("DELETE FROM indexed_roots WHERE id = ?1", params![root_id])?;
    Ok(())
}

pub fn lookup_root_path(conn: &Connection, root_id: i64) -> Result<Option<String>> {
    conn.query_row(
        "SELECT path FROM indexed_roots WHERE id = ?1",
        params![root_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn lookup_root_status(conn: &Connection, root_id: i64) -> Result<Option<String>> {
    conn.query_row(
        "SELECT status FROM indexed_roots WHERE id = ?1",
        params![root_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn lookup_root_record(conn: &Connection, root_id: i64) -> Result<Option<IndexedRoot>> {
    conn.query_row(
        &format!("{ROOT_SELECT} WHERE r.id = ?1"),
        params![root_id],
        map_root_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list_root_watch_entries(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut stmt =
        conn.prepare("SELECT id, path FROM indexed_roots ORDER BY path COLLATE NOCASE")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

pub fn mark_root_watch_state(
    conn: &Connection,
    root_id: i64,
    sync_status: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE indexed_roots
         SET sync_status = ?2,
             updated_at = ?3
         WHERE id = ?1",
        params![root_id, sync_status, now],
    )?;
    Ok(())
}

pub fn mark_root_change_detected(conn: &Connection, root_id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE indexed_roots
         SET sync_status = 'pending',
             last_change_at = ?2,
             updated_at = ?2
         WHERE id = ?1",
        params![root_id, now],
    )?;
    Ok(())
}

pub fn mark_root_syncing(conn: &Connection, root_id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE indexed_roots
         SET sync_status = 'syncing',
             updated_at = ?2
         WHERE id = ?1",
        params![root_id, now],
    )?;
    Ok(())
}

pub fn mark_root_synced(conn: &Connection, root_id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE indexed_roots
         SET sync_status = 'watching',
             last_synced_at = ?2,
             updated_at = ?2
         WHERE id = ?1",
        params![root_id, now],
    )?;
    Ok(())
}

pub fn refresh_root_file_count(conn: &Connection, root_id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE indexed_roots
         SET file_count = (
                SELECT COUNT(*)
                FROM files
                WHERE root_id = ?1
             ),
             updated_at = ?2
         WHERE id = ?1",
        params![root_id, now],
    )?;
    Ok(())
}

fn map_root_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedRoot> {
    Ok(IndexedRoot {
        id: row.get(0)?,
        path: row.get(1)?,
        status: row.get(2)?,
        sync_status: row.get(3)?,
        file_count: row.get(4)?,
        content_indexed_count: row.get(5)?,
        content_pending_count: row.get(6)?,
        semantic_indexed_count: row.get(7)?,
        semantic_pending_count: row.get(8)?,
        last_indexed_at: row.get(9)?,
        last_synced_at: row.get(10)?,
        last_change_at: row.get(11)?,
        last_error: row.get(12)?,
    })
}
