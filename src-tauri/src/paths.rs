//! Application directory layout. Everything lives in the per-user app data
//! directory unless the user configures an override for artwork storage.

use crate::error::Result;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub artwork_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve(app: &tauri::AppHandle) -> Result<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| crate::error::AppError::Io(std::io::Error::other(e)))?;
        let paths = AppPaths {
            db_path: data_dir.join("library.db"),
            artwork_dir: data_dir.join("artwork"),
            log_dir: data_dir.join("logs"),
            data_dir,
        };
        std::fs::create_dir_all(&paths.artwork_dir)?;
        std::fs::create_dir_all(&paths.log_dir)?;
        Ok(paths)
    }
}
