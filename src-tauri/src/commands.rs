use crate::{
    indexing,
    models::{FileDetails, IndexStatus, IndexedRoot, SearchRequest, SearchResult},
    semantic, shell,
    state::AppState,
    storage,
    utils::{err_to_string, unix_timestamp},
};
use tauri::State;

#[tauri::command]
pub fn list_index_roots(state: State<'_, AppState>) -> Result<Vec<IndexedRoot>, String> {
    let conn = state.connection().map_err(err_to_string)?;
    storage::fetch_roots(&conn).map_err(err_to_string)
}

#[tauri::command]
pub fn add_index_root(path: String, state: State<'_, AppState>) -> Result<IndexedRoot, String> {
    let normalized = indexing::normalize_root_path(&path).map_err(err_to_string)?;
    let conn = state.connection().map_err(err_to_string)?;
    storage::insert_or_update_root(&conn, &normalized, unix_timestamp()).map_err(err_to_string)
}

#[tauri::command]
pub fn remove_index_root(root_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let _ = semantic::remove_root_embeddings(&state.vector_db_path, root_id);
    let conn = state.connection().map_err(err_to_string)?;
    storage::remove_root(&conn, root_id).map_err(err_to_string)
}

#[tauri::command]
pub fn start_index(root_id: i64, state: State<'_, AppState>) -> Result<IndexStatus, String> {
    let conn = state.connection().map_err(err_to_string)?;
    let root_path = storage::lookup_root_path(&conn, root_id)
        .map_err(err_to_string)?
        .ok_or_else(|| "Indexed folder not found".to_string())?;

    let current_status = storage::lookup_root_status(&conn, root_id).map_err(err_to_string)?;
    if matches!(current_status.as_deref(), Some("indexing")) {
        let latest = storage::fetch_latest_job(&conn, root_id).map_err(err_to_string)?;
        return latest.ok_or_else(|| "Folder is already indexing".to_string());
    }

    let job_id =
        storage::create_index_job(&conn, root_id, unix_timestamp()).map_err(err_to_string)?;
    drop(conn);

    indexing::spawn_index_job(
        (*state.db_path).clone(),
        (*state.vector_db_path).clone(),
        (*state.model_cache_dir).clone(),
        root_id,
        job_id,
        root_path,
    );

    let conn = state.connection().map_err(err_to_string)?;
    storage::fetch_job_by_id(&conn, job_id)
        .map_err(err_to_string)?
        .ok_or_else(|| "Failed to create index job".to_string())
}

#[tauri::command]
pub fn get_index_statuses(state: State<'_, AppState>) -> Result<Vec<IndexStatus>, String> {
    let conn = state.connection().map_err(err_to_string)?;
    storage::fetch_latest_jobs(&conn).map_err(err_to_string)
}

#[tauri::command]
pub fn search_files(
    request: SearchRequest,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let conn = state.connection().map_err(err_to_string)?;
    let query = request.query.trim().to_lowercase();
    let limit = request.limit.unwrap_or(60).clamp(1, 200);
    crate::search::search_files(
        &conn,
        &state.vector_db_path,
        &state.model_cache_dir,
        &query,
        request.root_ids.as_deref(),
        limit,
    )
    .map_err(err_to_string)
}

#[tauri::command]
pub fn get_file_details(file_id: i64, state: State<'_, AppState>) -> Result<FileDetails, String> {
    let conn = state.connection().map_err(err_to_string)?;
    storage::fetch_file_details(&conn, file_id).map_err(err_to_string)
}

#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    shell::open_file(&path).map_err(err_to_string)
}

#[tauri::command]
pub fn reveal_file(path: String) -> Result<(), String> {
    shell::reveal_file(&path).map_err(err_to_string)
}
