use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedRoot {
    pub id: i64,
    pub path: String,
    pub status: String,
    pub file_count: i64,
    pub last_indexed_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub file_id: i64,
    pub root_id: i64,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub kind: String,
    pub size: i64,
    pub modified_at: Option<i64>,
    pub indexed_at: i64,
    pub score: i64,
    pub match_reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDetails {
    pub file_id: i64,
    pub root_id: i64,
    pub root_path: String,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub kind: String,
    pub size: i64,
    pub modified_at: Option<i64>,
    pub indexed_at: i64,
    pub preview_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub job_id: i64,
    pub root_id: i64,
    pub phase: String,
    pub status: String,
    pub processed: u64,
    pub total: u64,
    pub current_path: Option<String>,
    pub errors: Vec<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    pub root_ids: Option<Vec<i64>>,
    pub limit: Option<usize>,
}

#[derive(Debug)]
pub struct FileCandidate {
    pub file_id: i64,
    pub root_id: i64,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub kind: String,
    pub size: i64,
    pub modified_at: Option<i64>,
    pub indexed_at: i64,
}
