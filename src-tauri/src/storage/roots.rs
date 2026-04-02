use crate::models::IndexedRoot;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

const ROOT_SELECT: &str = "
    SELECT r.id,
           r.path,
           r.status,
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
        "INSERT INTO indexed_roots (path, status, file_count, created_at, updated_at)
         VALUES (?1, 'idle', 0, ?2, ?2)
         ON CONFLICT(path) DO UPDATE SET updated_at = excluded.updated_at",
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

fn map_root_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedRoot> {
    Ok(IndexedRoot {
        id: row.get(0)?,
        path: row.get(1)?,
        status: row.get(2)?,
        file_count: row.get(3)?,
        content_indexed_count: row.get(4)?,
        content_pending_count: row.get(5)?,
        semantic_indexed_count: row.get(6)?,
        semantic_pending_count: row.get(7)?,
        last_indexed_at: row.get(8)?,
        last_error: row.get(9)?,
    })
}
