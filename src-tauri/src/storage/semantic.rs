use crate::models::{FileSemanticPreview, SemanticSourceFile};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub fn replace_semantic_record(
    conn: &Connection,
    file_id: i64,
    status: &str,
    modality: Option<&str>,
    model: Option<&str>,
    summary: Option<&str>,
    updated_at: i64,
    error_message: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO file_semantic_index (
            file_id,
            status,
            modality,
            model,
            summary,
            updated_at,
            error_message
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(file_id) DO UPDATE SET
            status = excluded.status,
            modality = excluded.modality,
            model = excluded.model,
            summary = excluded.summary,
            updated_at = excluded.updated_at,
            error_message = excluded.error_message",
        params![
            file_id,
            status,
            modality,
            model,
            summary,
            updated_at,
            error_message
        ],
    )?;

    Ok(())
}

pub fn fetch_semantic_preview(conn: &Connection, file_id: i64) -> Result<FileSemanticPreview> {
    let preview = conn
        .query_row(
            "SELECT status, modality, model, summary, error_message
             FROM file_semantic_index
             WHERE file_id = ?1",
            params![file_id],
            |row| {
                Ok(FileSemanticPreview {
                    semantic_status: row.get(0)?,
                    semantic_modality: row.get(1)?,
                    semantic_model: row.get(2)?,
                    semantic_summary: row.get(3)?,
                    semantic_error: row.get(4)?,
                })
            },
        )
        .optional()?;

    Ok(preview.unwrap_or(FileSemanticPreview {
        semantic_status: None,
        semantic_modality: None,
        semantic_model: None,
        semantic_summary: None,
        semantic_error: None,
    }))
}

pub fn fetch_semantic_backfill_candidates(
    conn: &Connection,
    root_id: i64,
) -> Result<Vec<SemanticSourceFile>> {
    let mut stmt = conn.prepare(
        "SELECT f.id,
                f.root_id,
                f.path,
                f.kind,
                (
                    SELECT c.source_label
                    FROM file_text_chunks c
                    WHERE c.file_id = f.id
                    ORDER BY CAST(c.chunk_index AS INTEGER) ASC
                    LIMIT 1
                ) AS summary,
                (
                    SELECT group_concat(text, ' ')
                    FROM (
                        SELECT c.text
                        FROM file_text_chunks c
                        WHERE c.file_id = f.id
                        ORDER BY CAST(c.chunk_index AS INTEGER) ASC
                        LIMIT 6
                    )
                ) AS content_text
         FROM files f
         LEFT JOIN file_semantic_index s ON s.file_id = f.id
         WHERE f.root_id = ?1
           AND f.kind IN ('image', 'document', 'text', 'code')
           AND (s.status IS NULL OR s.status = 'pending')
         ORDER BY f.id ASC",
    )?;

    let rows = stmt.query_map(params![root_id], |row| {
        Ok(SemanticSourceFile {
            file_id: row.get(0)?,
            root_id: row.get(1)?,
            path: row.get(2)?,
            kind: row.get(3)?,
            summary: row.get(4)?,
            content_text: row.get(5)?,
        })
    })?;

    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row?);
    }
    Ok(candidates)
}
