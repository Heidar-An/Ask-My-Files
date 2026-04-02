use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedRoot {
    pub id: i64,
    pub path: String,
    pub status: String,
    pub file_count: i64,
    pub content_indexed_count: i64,
    pub content_pending_count: i64,
    pub semantic_indexed_count: i64,
    pub semantic_pending_count: i64,
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
    pub semantic_score: Option<f32>,
    pub match_reasons: Vec<String>,
    pub snippet: Option<String>,
    pub snippet_source: Option<String>,
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
    pub content_status: Option<String>,
    pub content_snippet: Option<String>,
    pub content_source: Option<String>,
    pub extraction_error: Option<String>,
    pub semantic_status: Option<String>,
    pub semantic_modality: Option<String>,
    pub semantic_model: Option<String>,
    pub semantic_summary: Option<String>,
    pub semantic_error: Option<String>,
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

#[derive(Debug, Clone)]
pub struct StoredFile {
    pub file_id: i64,
}

#[derive(Debug)]
pub struct ContentMatch {
    pub file_id: i64,
    pub root_id: i64,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub kind: String,
    pub size: i64,
    pub modified_at: Option<i64>,
    pub indexed_at: i64,
    pub source_label: Option<String>,
    pub text: String,
}

#[derive(Debug)]
pub struct FileContentPreview {
    pub content_status: Option<String>,
    pub content_snippet: Option<String>,
    pub content_source: Option<String>,
    pub extraction_error: Option<String>,
}

#[derive(Debug)]
pub struct FileSemanticPreview {
    pub semantic_status: Option<String>,
    pub semantic_modality: Option<String>,
    pub semantic_model: Option<String>,
    pub semantic_summary: Option<String>,
    pub semantic_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SemanticMatch {
    pub file_id: i64,
    pub modality: String,
    pub similarity: f32,
}

#[derive(Debug, Clone)]
pub struct SemanticSourceFile {
    pub file_id: i64,
    pub root_id: i64,
    pub path: String,
    pub kind: String,
    pub summary: Option<String>,
    pub content_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContentSourceFile {
    pub file_id: i64,
    pub path: String,
    pub extension: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct ExistingFileSnapshot {
    pub file_id: i64,
    pub path: String,
    pub size: i64,
    pub modified_at: Option<i64>,
}
