use crate::storage;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{fs, path::PathBuf, sync::Arc};
use tauri::{AppHandle, Manager};

const DATABASE_NAME: &str = "mira.db";
const VECTOR_DATABASE_DIR: &str = "semantic-index.lancedb";
const MODEL_CACHE_DIR: &str = "semantic-models";

#[derive(Clone)]
pub struct AppState {
    pub db_path: Arc<PathBuf>,
    pub vector_db_path: Arc<PathBuf>,
    pub model_cache_dir: Arc<PathBuf>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let app_dir = app
            .path()
            .app_local_data_dir()
            .context("failed to resolve app data directory")?;
        fs::create_dir_all(&app_dir).context("failed to create app data directory")?;

        let db_path = app_dir.join(DATABASE_NAME);
        let vector_db_path = app_dir.join(VECTOR_DATABASE_DIR);
        let model_cache_dir = app_dir.join(MODEL_CACHE_DIR);
        fs::create_dir_all(&vector_db_path).context("failed to create vector index directory")?;
        fs::create_dir_all(&model_cache_dir).context("failed to create model cache directory")?;
        storage::initialize_database(&db_path)?;

        Ok(Self {
            db_path: Arc::new(db_path),
            vector_db_path: Arc::new(vector_db_path),
            model_cache_dir: Arc::new(model_cache_dir),
        })
    }

    pub fn connection(&self) -> Result<Connection> {
        storage::open_connection(&self.db_path)
    }
}
