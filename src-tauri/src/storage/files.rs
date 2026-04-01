use crate::{
    models::{FileCandidate, FileDetails},
    preview::preview_path_for_kind,
};
use anyhow::{anyhow, Context, Result};
use rusqlite::{params, params_from_iter, types::Value, Connection};
use std::{fs, path::Path};

pub fn fetch_candidates(
    conn: &Connection,
    query: &str,
    root_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<FileCandidate>> {
    let mut sql = String::from(
        "SELECT id, root_id, name, path, extension, kind, size, modified_at, indexed_at FROM files",
    );
    let mut clauses = Vec::new();
    let mut values = Vec::<Value>::new();

    if let Some(root_ids) = root_ids {
        if !root_ids.is_empty() {
            clauses.push(format!(
                "root_id IN ({})",
                std::iter::repeat("?")
                    .take(root_ids.len())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            values.extend(root_ids.iter().copied().map(Value::Integer));
        }
    }

    if !query.is_empty() {
        let tokens = query
            .split_whitespace()
            .map(|token| token.trim_matches(|char: char| !char.is_alphanumeric()))
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        if !tokens.is_empty() {
            let mut token_clauses = Vec::new();
            for token in tokens {
                token_clauses.push(
                    "(lower(name) LIKE ? OR lower(path) LIKE ? OR lower(extension) = ?)"
                        .to_string(),
                );
                let like = format!("%{token}%");
                values.push(Value::Text(like.clone()));
                values.push(Value::Text(like));
                values.push(Value::Text(token));
            }
            clauses.push(format!("({})", token_clauses.join(" OR ")));
        }
    }

    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }

    sql.push_str(" ORDER BY modified_at DESC, indexed_at DESC LIMIT ?");
    values.push(Value::Integer((limit as i64) * 20));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(values.iter()), map_file_candidate)?;

    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    Ok(candidates)
}

pub fn fetch_file_details(conn: &Connection, file_id: i64) -> Result<FileDetails> {
    let details = conn.query_row(
        "SELECT f.id,
                f.root_id,
                r.path,
                f.name,
                f.path,
                f.extension,
                f.kind,
                f.size,
                f.modified_at,
                f.indexed_at
         FROM files f
         JOIN indexed_roots r ON r.id = f.root_id
         WHERE f.id = ?1",
        params![file_id],
        |row| {
            let path: String = row.get(4)?;
            let kind: String = row.get(6)?;
            Ok(FileDetails {
                file_id: row.get(0)?,
                root_id: row.get(1)?,
                root_path: row.get(2)?,
                name: row.get(3)?,
                path: path.clone(),
                extension: row.get(5)?,
                kind: kind.clone(),
                size: row.get(7)?,
                modified_at: row.get(8)?,
                indexed_at: row.get(9)?,
                preview_path: preview_path_for_kind(&path, &kind),
            })
        },
    )?;

    Ok(details)
}

pub fn delete_files_for_root(conn: &Connection, root_id: i64) -> Result<()> {
    conn.execute("DELETE FROM files WHERE root_id = ?1", params![root_id])?;
    Ok(())
}

pub fn index_file(conn: &Connection, root_id: i64, path: &Path, indexed_at: i64) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for {}", path.display()))?;
    let modified_at = metadata
        .modified()
        .ok()
        .map(crate::utils::system_time_to_timestamp);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| anyhow!("failed to read filename for {}", path.display()))?;
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    let kind = crate::indexing::classify_kind(&extension).to_string();

    conn.execute(
        "INSERT INTO files (
            root_id,
            path,
            name,
            extension,
            kind,
            size,
            modified_at,
            indexed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(path) DO UPDATE SET
            root_id = excluded.root_id,
            name = excluded.name,
            extension = excluded.extension,
            kind = excluded.kind,
            size = excluded.size,
            modified_at = excluded.modified_at,
            indexed_at = excluded.indexed_at",
        params![
            root_id,
            path.to_string_lossy().into_owned(),
            name,
            extension,
            kind,
            metadata.len() as i64,
            modified_at,
            indexed_at
        ],
    )?;

    Ok(())
}

pub fn map_file_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileCandidate> {
    Ok(FileCandidate {
        file_id: row.get(0)?,
        root_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        extension: row.get(4)?,
        kind: row.get(5)?,
        size: row.get(6)?,
        modified_at: row.get(7)?,
        indexed_at: row.get(8)?,
    })
}
