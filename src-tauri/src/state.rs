use crate::storage;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::{fs, path::PathBuf, sync::Arc};
use tauri::{AppHandle, Manager};

const DATABASE_NAME: &str = "ask-my-files.db";

#[derive(Clone)]
pub struct AppState {
    pub db_path: Arc<PathBuf>,
}

impl AppState {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let app_dir = app
            .path()
            .app_local_data_dir()
            .context("failed to resolve app data directory")?;
        fs::create_dir_all(&app_dir).context("failed to create app data directory")?;

        let db_path = app_dir.join(DATABASE_NAME);
        storage::initialize_database(&db_path)?;

        Ok(Self {
            db_path: Arc::new(db_path),
        })
    }

    pub fn connection(&self) -> Result<Connection> {
        storage::open_connection(&self.db_path)
    }
}
