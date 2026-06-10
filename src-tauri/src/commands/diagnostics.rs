use super::AppState;
use crate::error::Result;
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub version: String,
    pub counts: HashMap<String, i64>,
    pub data_dir: String,
    pub db_path: String,
    pub log_dir: String,
    pub db_size_bytes: u64,
    pub artwork_size_bytes: u64,
}

#[tauri::command]
pub async fn get_diagnostics(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Diagnostics> {
    let counts = state.db.with(|c| {
        let mut counts = HashMap::new();
        for table in [
            "games",
            "installations",
            "emulators",
            "platforms",
            "artwork",
            "play_sessions",
            "collections",
            "rom_directories",
        ] {
            let n: i64 = c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            counts.insert(table.to_string(), n);
        }
        Ok(counts)
    })?;

    let db_size_bytes = std::fs::metadata(&state.paths.db_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let artwork_size_bytes = dir_size(&state.paths.artwork_dir);

    Ok(Diagnostics {
        version: app.package_info().version.to_string(),
        counts,
        data_dir: state.paths.data_dir.to_string_lossy().to_string(),
        db_path: state.paths.db_path.to_string_lossy().to_string(),
        log_dir: state.paths.log_dir.to_string_lossy().to_string(),
        db_size_bytes,
        artwork_size_bytes,
    })
}

fn dir_size(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Tail the current log file.
#[tauri::command]
pub async fn read_logs(state: State<'_, AppState>, lines: Option<usize>) -> Result<Vec<String>> {
    let limit = lines.unwrap_or(200).min(2000);
    let mut entries: Vec<_> = std::fs::read_dir(&state.paths.log_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    let latest = match entries.last() {
        Some(e) => e.path(),
        None => return Ok(vec![]),
    };
    let content = std::fs::read_to_string(latest)?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(limit);
    Ok(all[start..].iter().map(|s| s.to_string()).collect())
}
