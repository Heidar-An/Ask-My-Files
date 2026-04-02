mod content;
mod files;
mod jobs;
mod roots;
mod schema;
mod semantic;

pub use content::{
    fetch_content_backfill_candidates, fetch_content_preview, replace_file_content,
    search_content_matches,
};
pub use files::{
    delete_files_by_ids, fetch_candidates, fetch_candidates_by_ids, fetch_file_details,
    fetch_root_file_snapshots, index_file,
};
pub use jobs::{
    create_index_job, fetch_job_by_id, fetch_latest_job, fetch_latest_jobs, mark_job_failed,
    update_job_progress, update_root_ready,
};
pub use roots::{
    fetch_roots, insert_or_update_root, lookup_root_path, lookup_root_status, remove_root,
};
pub use schema::{initialize_database, open_connection};
pub use semantic::{
    fetch_semantic_backfill_candidates, fetch_semantic_preview, replace_semantic_record,
};
