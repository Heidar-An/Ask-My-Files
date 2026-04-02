use crate::{
    extractors::ExtractionOutput,
    models::{ContentMatch, ContentSourceFile, FileContentPreview},
};
use anyhow::Result;
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};

pub fn replace_file_content(
    conn: &Connection,
    file_id: i64,
    output: &ExtractionOutput,
    indexed_at: i64,
) -> Result<()> {
    conn.execute(
        "DELETE FROM file_text_chunks WHERE file_id = ?1",
        params![file_id],
    )?;

    conn.execute(
        "INSERT INTO file_extracts (
            file_id,
            status,
            extractor,
            text_length,
            chunk_count,
            updated_at,
            error_message
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(file_id) DO UPDATE SET
            status = excluded.status,
            extractor = excluded.extractor,
            text_length = excluded.text_length,
            chunk_count = excluded.chunk_count,
            updated_at = excluded.updated_at,
            error_message = excluded.error_message",
        params![
            file_id,
            output.status,
            output.extractor.as_deref(),
            output.text_length,
            output.chunks.len() as i64,
            indexed_at,
            output.error_message.as_deref()
        ],
    )?;

    if output.chunks.is_empty() {
        return Ok(());
    }

    let mut insert_chunk = conn.prepare_cached(
        "INSERT INTO file_text_chunks (file_id, chunk_index, source_label, text)
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for (chunk_index, chunk) in output.chunks.iter().enumerate() {
        insert_chunk.execute(params![
            file_id,
            chunk_index as i64,
            chunk.source_label.as_deref(),
            &chunk.text
        ])?;
    }

    Ok(())
}

pub fn fetch_content_preview(conn: &Connection, file_id: i64) -> Result<FileContentPreview> {
    let preview = conn
        .query_row(
            "SELECT e.status,
                    c.text,
                    c.source_label,
                    e.error_message
             FROM file_extracts e
             LEFT JOIN file_text_chunks c ON c.file_id = e.file_id
             WHERE e.file_id = ?1
             ORDER BY CAST(c.chunk_index AS INTEGER) ASC
             LIMIT 1",
            params![file_id],
            |row| {
                let text: Option<String> = row.get(1)?;
                Ok(FileContentPreview {
                    content_status: row.get(0)?,
                    content_snippet: text.as_deref().map(truncate_preview),
                    content_source: row.get(2)?,
                    extraction_error: row.get(3)?,
                })
            },
        )
        .optional()?;

    Ok(preview.unwrap_or(FileContentPreview {
        content_status: None,
        content_snippet: None,
        content_source: None,
        extraction_error: None,
    }))
}

pub fn search_content_matches(
    conn: &Connection,
    fts_query: &str,
    root_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<ContentMatch>> {
    let mut sql = String::from(
        "SELECT f.id,
                f.root_id,
                f.name,
                f.path,
                f.extension,
                f.kind,
                f.size,
                f.modified_at,
                f.indexed_at,
                file_text_chunks.source_label,
                file_text_chunks.text
         FROM file_text_chunks
         JOIN files f ON f.id = file_text_chunks.file_id
         WHERE file_text_chunks MATCH ?",
    );
    let mut values = vec![Value::Text(fts_query.to_string())];

    if let Some(root_ids) = root_ids {
        if !root_ids.is_empty() {
            sql.push_str(" AND f.root_id IN (");
            sql.push_str(
                &std::iter::repeat("?")
                    .take(root_ids.len())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sql.push(')');
            values.extend(root_ids.iter().copied().map(Value::Integer));
        }
    }

    sql.push_str(" ORDER BY bm25(file_text_chunks), f.modified_at DESC LIMIT ?");
    values.push(Value::Integer(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
        Ok(ContentMatch {
            file_id: row.get(0)?,
            root_id: row.get(1)?,
            name: row.get(2)?,
            path: row.get(3)?,
            extension: row.get(4)?,
            kind: row.get(5)?,
            size: row.get(6)?,
            modified_at: row.get(7)?,
            indexed_at: row.get(8)?,
            source_label: row.get(9)?,
            text: row.get(10)?,
        })
    })?;

    let mut matches = Vec::new();
    for row in rows {
        matches.push(row?);
    }
    Ok(matches)
}

pub fn fetch_content_backfill_candidates(
    conn: &Connection,
    root_id: i64,
) -> Result<Vec<ContentSourceFile>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.path, f.extension, f.kind
         FROM files f
         JOIN file_extracts e ON e.file_id = f.id
         WHERE f.root_id = ?1
           AND e.status = 'pending'
         ORDER BY f.id ASC",
    )?;

    let rows = stmt.query_map(params![root_id], |row| {
        Ok(ContentSourceFile {
            file_id: row.get(0)?,
            path: row.get(1)?,
            extension: row.get(2)?,
            kind: row.get(3)?,
        })
    })?;

    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    Ok(candidates)
}

fn truncate_preview(text: &str) -> String {
    const MAX_CHARS: usize = 220;

    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }

    let mut preview = text.chars().take(MAX_CHARS).collect::<String>();
    preview.push('…');
    preview
}
