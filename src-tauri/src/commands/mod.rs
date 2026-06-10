//! Tauri command surface. Commands are thin wrappers: they resolve state,
//! delegate to the domain modules, and translate errors.

pub mod diagnostics;
pub mod emulators;
pub mod launching;
pub mod library;
pub mod meta;
pub mod scanning;
pub mod settings_cmd;

use crate::db::Db;
use crate::paths::AppPaths;
use std::sync::Arc;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub db: Arc<Db>,
    pub paths: AppPaths,
}
