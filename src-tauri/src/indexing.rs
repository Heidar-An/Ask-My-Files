use crate::{
    extractors,
    models::ExistingFileSnapshot,
    semantic::{self, SemanticPlan},
    storage,
    utils::unix_timestamp,
};
use anyhow::{anyhow, Context, Result};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

const PROGRESS_UPDATE_EVERY: u64 = 25;
const INDEX_BATCH_SIZE: usize = 32;
const CONTENT_BACKFILL_BATCH_SIZE: usize = 12;
const INCREMENTAL_SEMANTIC_BATCH_SIZE: usize = 48;

struct PreparedFile {
    path: PathBuf,
    kind: String,
    indexed_at: i64,
    size: i64,
    modified_at: Option<i64>,
    content: extractors::ExtractionOutput,
    semantic_plan: SemanticPlan,
}

pub fn normalize_root_path(path: &str) -> Result<String> {
    let input = PathBuf::from(path);
    let canonical = fs::canonicalize(&input)
        .with_context(|| format!("failed to open folder {}", input.display()))?;
    if !canonical.is_dir() {
        return Err(anyhow!("selected path is not a folder"));
    }
    Ok(canonical.to_string_lossy().into_owned())
}

pub fn spawn_index_job(
    db_path: PathBuf,
    vector_db_path: PathBuf,
    model_cache_dir: PathBuf,
    root_id: i64,
    job_id: i64,
    root_path: String,
) {
    std::thread::spawn(move || {
        if let Err(error) = run_index_job(
            &db_path,
            &vector_db_path,
            &model_cache_dir,
            root_id,
            job_id,
            &root_path,
        ) {
            let _ = storage::mark_job_failed(&db_path, root_id, job_id, &error.to_string());
        }
    });
}

pub fn spawn_incremental_sync_job(
    db_path: PathBuf,
    vector_db_path: PathBuf,
    model_cache_dir: PathBuf,
    root_id: i64,
    root_path: String,
    changed_paths: Vec<String>,
    removed_paths: Vec<String>,
) {
    std::thread::spawn(move || {
        if let Err(error) = run_incremental_sync_job(
            &db_path,
            &vector_db_path,
            &model_cache_dir,
            root_id,
            &root_path,
            &changed_paths,
            &removed_paths,
        ) {
            if let Ok(conn) = storage::open_connection(&db_path) {
                let _ = storage::mark_root_watch_state(&conn, root_id, "error", unix_timestamp());
                let _ = storage::refresh_root_file_count(&conn, root_id, unix_timestamp());
                let _ = conn.execute(
                    "UPDATE indexed_roots
                     SET last_error = ?2,
                         updated_at = ?3
                     WHERE id = ?1",
                    rusqlite::params![root_id, error.to_string(), unix_timestamp()],
                );
            }
        }
    });
}

pub fn run_index_job(
    db_path: &Path,
    vector_db_path: &Path,
    model_cache_dir: &Path,
    root_id: i64,
    job_id: i64,
    root_path: &str,
) -> Result<()> {
    let root = PathBuf::from(root_path);
    if !root.exists() {
        return Err(anyhow!("folder does not exist anymore"));
    }

    let files = collect_files(&root)?;
    let total = files.len() as u64;
    storage::update_job_progress(db_path, root_id, job_id, 0, total, None)?;

    let mut conn = storage::open_connection(db_path)?;
    let existing_by_path = storage::fetch_root_file_snapshots(&conn, root_id)?
        .into_iter()
        .map(|snapshot| (snapshot.path.clone(), snapshot))
        .collect::<HashMap<_, _>>();

    let current_paths = files
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<HashSet<_>>();
    let removed_file_ids = existing_by_path
        .values()
        .filter(|snapshot| !current_paths.contains(&snapshot.path))
        .map(|snapshot| snapshot.file_id)
        .collect::<Vec<_>>();

    if !removed_file_ids.is_empty() {
        let tx = conn.transaction()?;
        storage::delete_files_by_ids(&tx, &removed_file_ids)?;
        tx.commit()?;
        let _ = semantic::remove_embeddings_for_files(vector_db_path, &removed_file_ids);
    }

    let mut processed = 0_u64;
    let mut last_error: Option<String> = None;
    let mut changed_file_ids = Vec::<i64>::new();

    for batch in files.chunks(INDEX_BATCH_SIZE) {
        let prepared_batch = batch
            .par_iter()
            .map(|path| prepare_file(path))
            .collect::<Vec<_>>();

        let tx = conn.transaction()?;
        for prepared in &prepared_batch {
            let path_string = prepared.path.to_string_lossy().into_owned();
            let existing = existing_by_path.get(&path_string);

            if is_unchanged(existing, prepared) {
                continue;
            }

            match storage::index_file(&tx, root_id, &prepared.path, prepared.indexed_at) {
                Ok(stored_file) => {
                    changed_file_ids.push(stored_file.file_id);

                    if let Err(error) = storage::replace_file_content(
                        &tx,
                        stored_file.file_id,
                        &prepared.content,
                        prepared.indexed_at,
                    ) {
                        last_error = Some(error.to_string());
                    }

                    if let Err(error) = storage::replace_semantic_record(
                        &tx,
                        stored_file.file_id,
                        &prepared.semantic_plan.status,
                        prepared.semantic_plan.modality.as_deref(),
                        None,
                        prepared.semantic_plan.summary.as_deref(),
                        prepared.indexed_at,
                        None,
                    ) {
                        last_error = Some(error.to_string());
                    }
                }
                Err(error) => {
                    last_error = Some(error.to_string());
                }
            }
        }
        tx.commit()?;

        processed += prepared_batch.len() as u64;
        let should_update_progress =
            processed % PROGRESS_UPDATE_EVERY <= prepared_batch.len() as u64 || processed == total;
        if should_update_progress {
            let current_path = prepared_batch
                .last()
                .map(|prepared| prepared.path.to_string_lossy().into_owned());
            storage::update_job_progress(db_path, root_id, job_id, processed, total, current_path)?;
        }
    }

    storage::update_root_ready(&conn, root_id, job_id, processed, total, last_error)?;
    drop(conn);

    let mut file_ids_to_requeue = changed_file_ids;
    file_ids_to_requeue.extend(removed_file_ids);
    if !file_ids_to_requeue.is_empty() {
        let _ = semantic::remove_embeddings_for_files(vector_db_path, &file_ids_to_requeue);
    }

    spawn_content_backfill_job(
        db_path.to_path_buf(),
        vector_db_path.to_path_buf(),
        model_cache_dir.to_path_buf(),
        root_id,
        job_id,
    );

    Ok(())
}

fn run_incremental_sync_job(
    db_path: &Path,
    vector_db_path: &Path,
    model_cache_dir: &Path,
    root_id: i64,
    root_path: &str,
    changed_paths: &[String],
    removed_paths: &[String],
) -> Result<()> {
    let root = PathBuf::from(root_path);
    if !root.exists() {
        return Err(anyhow!("folder does not exist anymore"));
    }

    let mut conn = storage::open_connection(db_path)?;
    let now = unix_timestamp();
    storage::mark_root_syncing(&conn, root_id, now)?;

    let removed_file_ids = storage::fetch_file_ids_by_paths(&conn, root_id, removed_paths)?;
    if !removed_file_ids.is_empty() {
        let tx = conn.transaction()?;
        storage::delete_files_by_ids(&tx, &removed_file_ids)?;
        tx.commit()?;
        let _ = semantic::remove_embeddings_for_files(vector_db_path, &removed_file_ids);
    }

    let normalized_changed_paths = changed_paths
        .iter()
        .filter_map(|path| {
            let path = PathBuf::from(path);
            if !path.exists() || !path.is_file() || !path.starts_with(&root) || is_ignored(&path) {
                return None;
            }
            Some(path)
        })
        .collect::<Vec<_>>();

    let existing_by_path = storage::fetch_file_snapshots_by_paths(
        &conn,
        root_id,
        &normalized_changed_paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    )?
    .into_iter()
    .map(|snapshot| (snapshot.path.clone(), snapshot))
    .collect::<HashMap<_, _>>();

    let prepared_batch = normalized_changed_paths
        .par_iter()
        .map(|path| prepare_incremental_file(path))
        .collect::<Vec<_>>();

    let mut semantic_items = Vec::new();
    let mut file_ids_to_requeue = removed_file_ids;

    {
        let tx = conn.transaction()?;
        for prepared in &prepared_batch {
            let path_string = prepared.path.to_string_lossy().into_owned();
            let existing = existing_by_path.get(&path_string);
            if is_unchanged(existing, prepared) {
                continue;
            }

            let stored_file =
                storage::index_file(&tx, root_id, &prepared.path, prepared.indexed_at)?;
            file_ids_to_requeue.push(stored_file.file_id);
            storage::replace_file_content(
                &tx,
                stored_file.file_id,
                &prepared.content,
                prepared.indexed_at,
            )?;
            storage::replace_semantic_record(
                &tx,
                stored_file.file_id,
                &prepared.semantic_plan.status,
                prepared.semantic_plan.modality.as_deref(),
                None,
                prepared.semantic_plan.summary.as_deref(),
                prepared.indexed_at,
                None,
            )?;

            if let Some(item) = semantic::build_index_item_for_file(
                stored_file.file_id,
                root_id,
                &prepared.path,
                &prepared.kind,
                Some(&prepared.content),
            ) {
                semantic_items.push(item);
            }
        }
        tx.commit()?;
    }

    if !file_ids_to_requeue.is_empty() {
        let _ = semantic::remove_embeddings_for_files(vector_db_path, &file_ids_to_requeue);
    }

    if !semantic_items.is_empty() {
        let mut handle = semantic::open_index_handle(vector_db_path)?;
        for batch in semantic_items.chunks(INCREMENTAL_SEMANTIC_BATCH_SIZE) {
            match semantic::index_batch_with_handle(&mut handle, model_cache_dir, batch) {
                Ok(records) => {
                    let tx = conn.transaction()?;
                    let indexed_at = unix_timestamp();
                    for record in records {
                        storage::replace_semantic_record(
                            &tx,
                            record.file_id,
                            &record.status,
                            record.modality.as_deref(),
                            record.model.as_deref(),
                            record.summary.as_deref(),
                            indexed_at,
                            record.error_message.as_deref(),
                        )?;
                    }
                    tx.commit()?;
                }
                Err(error) => {
                    let tx = conn.transaction()?;
                    let indexed_at = unix_timestamp();
                    for item in batch {
                        storage::replace_semantic_record(
                            &tx,
                            item.file_id,
                            "error",
                            Some(&item.modality),
                            Some(semantic::semantic_model_name()),
                            item.summary.as_deref(),
                            indexed_at,
                            Some(&error.to_string()),
                        )?;
                    }
                    tx.commit()?;
                    break;
                }
            }
        }
    }

    let synced_at = unix_timestamp();
    storage::refresh_root_file_count(&conn, root_id, synced_at)?;
    storage::mark_root_synced(&conn, root_id, synced_at)?;
    Ok(())
}

pub fn classify_kind(extension: &str) -> &'static str {
    match extension {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "bmp" | "tiff" => "image",
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx" | "rtf" | "pages" => "document",
        "md" | "txt" | "json" | "csv" | "log" | "yaml" | "yml" | "toml" | "xml" => "text",
        "ts" | "tsx" | "js" | "jsx" | "rs" | "py" | "go" | "java" | "rb" | "css" | "html"
        | "sql" | "sh" => "code",
        "zip" | "tar" | "gz" | "rar" | "7z" => "archive",
        "mp3" | "wav" | "aac" | "m4a" | "flac" => "audio",
        "mp4" | "mov" | "mkv" | "avi" | "webm" => "video",
        _ => "other",
    }
}

fn spawn_content_backfill_job(
    db_path: PathBuf,
    vector_db_path: PathBuf,
    model_cache_dir: PathBuf,
    root_id: i64,
    job_id: i64,
) {
    std::thread::spawn(move || {
        let _ =
            run_content_backfill_job(&db_path, &vector_db_path, &model_cache_dir, root_id, job_id);
    });
}

fn run_content_backfill_job(
    db_path: &Path,
    vector_db_path: &Path,
    model_cache_dir: &Path,
    root_id: i64,
    job_id: i64,
) -> Result<()> {
    let mut conn = storage::open_connection(db_path)?;
    if !job_is_current(&conn, root_id, job_id)? {
        return Ok(());
    }

    let candidates = storage::fetch_content_backfill_candidates(&conn, root_id)?;
    if candidates.is_empty() {
        drop(conn);
        spawn_semantic_backfill_job(
            db_path.to_path_buf(),
            vector_db_path.to_path_buf(),
            model_cache_dir.to_path_buf(),
            root_id,
            job_id,
        );
        return Ok(());
    }

    for batch in candidates.chunks(CONTENT_BACKFILL_BATCH_SIZE) {
        if !job_is_current(&conn, root_id, job_id)? {
            return Ok(());
        }

        let extracted_batch = batch
            .par_iter()
            .map(|candidate| {
                let path = PathBuf::from(&candidate.path);
                let output =
                    extractors::extract_file_text(&path, &candidate.kind, &candidate.extension);
                (candidate.file_id, output)
            })
            .collect::<Vec<_>>();

        let tx = conn.transaction()?;
        for (file_id, output) in extracted_batch {
            storage::replace_file_content(&tx, file_id, &output, unix_timestamp())?;
            if output.status != "indexed" {
                storage::replace_semantic_record(
                    &tx,
                    file_id,
                    &output.status,
                    Some("text"),
                    None,
                    None,
                    unix_timestamp(),
                    output.error_message.as_deref(),
                )?;
            }
        }
        tx.commit()?;
    }

    drop(conn);
    spawn_semantic_backfill_job(
        db_path.to_path_buf(),
        vector_db_path.to_path_buf(),
        model_cache_dir.to_path_buf(),
        root_id,
        job_id,
    );

    Ok(())
}

fn spawn_semantic_backfill_job(
    db_path: PathBuf,
    vector_db_path: PathBuf,
    model_cache_dir: PathBuf,
    root_id: i64,
    job_id: i64,
) {
    std::thread::spawn(move || {
        let _ =
            run_semantic_backfill_job(&db_path, &vector_db_path, &model_cache_dir, root_id, job_id);
    });
}

fn run_semantic_backfill_job(
    db_path: &Path,
    vector_db_path: &Path,
    model_cache_dir: &Path,
    root_id: i64,
    job_id: i64,
) -> Result<()> {
    let mut conn = storage::open_connection(db_path)?;
    if !job_is_current(&conn, root_id, job_id)? {
        return Ok(());
    }

    let candidates = storage::fetch_semantic_backfill_candidates(&conn, root_id)?;
    if candidates.is_empty() {
        return Ok(());
    }

    let items = candidates
        .iter()
        .filter_map(semantic::build_index_item)
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Ok(());
    }

    let mut text_items = Vec::new();
    let mut image_items = Vec::new();

    for item in items {
        match item.modality.as_str() {
            "text" => text_items.push(item),
            "image" => image_items.push(item),
            _ => {}
        }
    }

    let mut handle = semantic::open_index_handle(vector_db_path)?;
    process_semantic_items(
        &mut conn,
        &mut handle,
        model_cache_dir,
        root_id,
        job_id,
        &text_items,
        semantic::SEMANTIC_TEXT_BATCH_SIZE,
    )?;
    process_semantic_items(
        &mut conn,
        &mut handle,
        model_cache_dir,
        root_id,
        job_id,
        &image_items,
        semantic::SEMANTIC_IMAGE_BATCH_SIZE,
    )?;

    Ok(())
}

fn process_semantic_items(
    conn: &mut rusqlite::Connection,
    handle: &mut semantic::SemanticIndexHandle,
    model_cache_dir: &Path,
    root_id: i64,
    job_id: i64,
    items: &[semantic::SemanticIndexItem],
    batch_size: usize,
) -> Result<()> {
    for batch in items.chunks(batch_size.max(1)) {
        if !job_is_current(conn, root_id, job_id)? {
            return Ok(());
        }

        match semantic::index_batch_with_handle(handle, model_cache_dir, batch) {
            Ok(records) => {
                let tx = conn.transaction()?;
                let indexed_at = unix_timestamp();
                for record in records {
                    storage::replace_semantic_record(
                        &tx,
                        record.file_id,
                        &record.status,
                        record.modality.as_deref(),
                        record.model.as_deref(),
                        record.summary.as_deref(),
                        indexed_at,
                        record.error_message.as_deref(),
                    )?;
                }
                tx.commit()?;
            }
            Err(error) => {
                let tx = conn.transaction()?;
                let indexed_at = unix_timestamp();
                for item in batch {
                    storage::replace_semantic_record(
                        &tx,
                        item.file_id,
                        "error",
                        Some(&item.modality),
                        Some(semantic::semantic_model_name()),
                        item.summary.as_deref(),
                        indexed_at,
                        Some(&error.to_string()),
                    )?;
                }
                tx.commit()?;
                return Ok(());
            }
        }
    }

    Ok(())
}

fn job_is_current(conn: &rusqlite::Connection, root_id: i64, job_id: i64) -> Result<bool> {
    Ok(storage::fetch_latest_job(conn, root_id)?
        .map(|job| job.job_id == job_id)
        .unwrap_or(false))
}

fn prepare_file(path: &Path) -> PreparedFile {
    let indexed_at = unix_timestamp();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    let kind = classify_kind(&extension).to_string();
    let metadata = fs::metadata(path).ok();
    let size = metadata
        .as_ref()
        .map(|metadata| metadata.len() as i64)
        .unwrap_or(0);
    let modified_at = metadata
        .and_then(|metadata| metadata.modified().ok())
        .map(crate::utils::system_time_to_timestamp);

    PreparedFile {
        path: path.to_path_buf(),
        kind: kind.clone(),
        indexed_at,
        size,
        modified_at,
        content: extractors::placeholder_output(&kind, &extension),
        semantic_plan: semantic::prepare_semantic_plan(&kind),
    }
}

fn prepare_incremental_file(path: &Path) -> PreparedFile {
    let indexed_at = unix_timestamp();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    let kind = classify_kind(&extension).to_string();
    let metadata = fs::metadata(path).ok();
    let size = metadata
        .as_ref()
        .map(|metadata| metadata.len() as i64)
        .unwrap_or(0);
    let modified_at = metadata
        .and_then(|metadata| metadata.modified().ok())
        .map(crate::utils::system_time_to_timestamp);
    let content = extractors::extract_file_text(path, &kind, &extension);

    PreparedFile {
        path: path.to_path_buf(),
        kind: kind.clone(),
        indexed_at,
        size,
        modified_at,
        content,
        semantic_plan: semantic::prepare_semantic_plan(&kind),
    }
}

fn is_unchanged(existing: Option<&ExistingFileSnapshot>, prepared: &PreparedFile) -> bool {
    existing
        .map(|existing| {
            existing.size == prepared.size && existing.modified_at == prepared.modified_at
        })
        .unwrap_or(false)
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        match entry {
            Ok(entry) if entry.file_type().is_file() => files.push(entry.into_path()),
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
    }

    Ok(files)
}

pub(crate) fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| matches!(name, ".git" | "node_modules" | "target" | ".DS_Store"))
        .unwrap_or(false)
}
