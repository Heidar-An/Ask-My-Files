use crate::{storage, utils::unix_timestamp};
use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

const PROGRESS_UPDATE_EVERY: u64 = 25;

pub fn normalize_root_path(path: &str) -> Result<String> {
    let input = PathBuf::from(path);
    let canonical = fs::canonicalize(&input)
        .with_context(|| format!("failed to open folder {}", input.display()))?;
    if !canonical.is_dir() {
        return Err(anyhow!("selected path is not a folder"));
    }
    Ok(canonical.to_string_lossy().into_owned())
}

pub fn spawn_index_job(db_path: PathBuf, root_id: i64, job_id: i64, root_path: String) {
    std::thread::spawn(move || {
        if let Err(error) = run_index_job(&db_path, root_id, job_id, &root_path) {
            let _ = storage::mark_job_failed(&db_path, root_id, job_id, &error.to_string());
        }
    });
}

pub fn run_index_job(db_path: &Path, root_id: i64, job_id: i64, root_path: &str) -> Result<()> {
    let root = PathBuf::from(root_path);
    if !root.exists() {
        return Err(anyhow!("folder does not exist anymore"));
    }

    let total = count_files(&root);
    storage::update_job_progress(db_path, root_id, job_id, 0, total, None)?;

    let conn = storage::open_connection(db_path)?;
    storage::delete_files_for_root(&conn, root_id)?;

    let mut processed = 0_u64;
    let mut last_error: Option<String> = None;

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        match entry {
            Ok(entry) if entry.file_type().is_file() => {
                let path = entry.path();
                match storage::index_file(&conn, root_id, path, unix_timestamp()) {
                    Ok(()) => {}
                    Err(error) => {
                        last_error = Some(error.to_string());
                    }
                }

                processed += 1;
                if processed % PROGRESS_UPDATE_EVERY == 0 || processed == total {
                    storage::update_job_progress(
                        db_path,
                        root_id,
                        job_id,
                        processed,
                        total,
                        Some(path.to_string_lossy().into_owned()),
                    )?;
                }
            }
            Ok(_) => {}
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }
    }

    storage::update_root_ready(&conn, root_id, job_id, processed, total, last_error)?;
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

fn count_files(root: &Path) -> u64 {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .count() as u64
}

fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| matches!(name, ".git" | "node_modules" | "target" | ".DS_Store"))
        .unwrap_or(false)
}
