mod files;
mod jobs;
mod roots;
mod schema;

pub use files::{delete_files_for_root, fetch_candidates, fetch_file_details, index_file};
pub use jobs::{
    create_index_job, fetch_job_by_id, fetch_latest_job, fetch_latest_jobs, mark_job_failed,
    update_job_progress, update_root_ready,
};
pub use roots::{
    fetch_roots, insert_or_update_root, lookup_root_path, lookup_root_status, remove_root,
};
pub use schema::{initialize_database, open_connection};
