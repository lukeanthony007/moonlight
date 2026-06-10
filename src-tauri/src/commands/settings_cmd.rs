use super::AppState;
use crate::db::repo::settings;
use crate::error::{AppError, Result};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Value> {
    state.db.with(settings::all)
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: Value) -> Result<()> {
    state.db.with(|c| settings::set(c, &key, &value))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub artwork_dir: String,
    pub log_dir: String,
    pub onboarding_complete: bool,
}

#[tauri::command]
pub async fn get_app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<AppInfo> {
    let onboarding_complete = state
        .db
        .with(|c| settings::get_bool(c, "general.onboardingComplete", false))?;
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.paths.data_dir.to_string_lossy().to_string(),
        db_path: state.paths.db_path.to_string_lossy().to_string(),
        artwork_dir: state.paths.artwork_dir.to_string_lossy().to_string(),
        log_dir: state.paths.log_dir.to_string_lossy().to_string(),
        onboarding_complete,
    })
}

#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> Result<()> {
    state
        .db
        .with(|c| settings::set(c, "general.onboardingComplete", &Value::Bool(true)))
}

/// Copy the database (after a WAL checkpoint) to a destination of the user's
/// choosing.
#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>, dest_path: String) -> Result<String> {
    if dest_path.trim().is_empty() {
        return Err(AppError::Invalid("destination path is required".into()));
    }
    state.db.checkpoint()?;
    std::fs::copy(&state.paths.db_path, &dest_path)?;
    tracing::info!(dest = %dest_path, "database backup created");
    Ok(dest_path)
}

/// Export the library (games, installations, platforms, emulators, artwork
/// records, sessions, collections, settings) as a single JSON document.
#[tauri::command]
pub async fn export_library(state: State<'_, AppState>, dest_path: String) -> Result<String> {
    use crate::db::repo::{collections, emulators, games, platforms, rom_dirs};
    let export = state.db.with(|c| {
        Ok(serde_json::json!({
            "exportedAt": crate::db::repo::now(),
            "version": 1,
            "games": games::list_library(c)?,
            "platforms": platforms::list(c)?,
            "emulators": emulators::list(c)?,
            "romDirectories": rom_dirs::list(c)?,
            "collections": collections::list(c)?,
            "settings": settings::all(c)?,
        }))
    })?;
    std::fs::write(&dest_path, serde_json::to_string_pretty(&export).unwrap())?;
    Ok(dest_path)
}
