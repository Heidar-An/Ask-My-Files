use crate::{storage, watchers::RootWatchService};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{fs, path::PathBuf, sync::Arc};
use tauri::{AppHandle, Manager};

const DATABASE_NAME: &str = "mira.db";
const VECTOR_DATABASE_DIR: &str = "semantic-index.lancedb";
const MODEL_CACHE_DIR: &str = "semantic-models";

#[derive(Clone)]
pub struct AppState {
    pub app: AppHandle,
    pub db_path: Arc<PathBuf>,
    pub vector_db_path: Arc<PathBuf>,
    pub model_cache_dir: Arc<PathBuf>,
    pub watch_service: Arc<RootWatchService>,
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
        let watch_service = Arc::new(RootWatchService::new(
            db_path.clone(),
            vector_db_path.clone(),
            model_cache_dir.clone(),
        )?);

        let conn = storage::open_connection(&db_path)?;
        for (root_id, path) in storage::list_root_watch_entries(&conn)? {
            app.asset_protocol_scope()
                .allow_directory(&path, true)
                .with_context(|| format!("failed to allow asset access for {}", path))?;
            watch_service.watch_root(root_id, path);
        }

        Ok(Self {
            app: app.clone(),
            db_path: Arc::new(db_path),
            vector_db_path: Arc::new(vector_db_path),
            model_cache_dir: Arc::new(model_cache_dir),
            watch_service,
        })
    }

    pub fn connection(&self) -> Result<Connection> {
        storage::open_connection(&self.db_path)
    }

    pub fn allow_preview_root(&self, path: &str) -> Result<()> {
        self.app
            .asset_protocol_scope()
            .allow_directory(path, true)
            .with_context(|| format!("failed to allow preview access for {}", path))
    }
}
