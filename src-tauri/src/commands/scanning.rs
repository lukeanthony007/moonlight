use super::AppState;
use crate::domain::ScanReport;
use crate::error::{AppError, Result};
use crate::metadata;
use crate::scan::{run_scan, ScanRegistry, ScanScope};
use serde::Deserialize;
use tauri::{Emitter, Manager, State};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScanScopeInput {
    All,
    Steam,
    #[serde(rename_all = "camelCase")]
    Directory {
        directory_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Platform {
        platform_id: String,
    },
}

/// Start a scan on a background thread. Progress arrives via `scan-progress`
/// events and the final report via `scan-complete`.
#[tauri::command]
pub async fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    scope: ScanScopeInput,
) -> Result<String> {
    let scan_id = uuid::Uuid::new_v4().to_string();
    let registry = app.state::<ScanRegistry>();
    let cancel = registry.begin(&scan_id);

    let db = state.db.clone();
    let artwork_dir = state.paths.artwork_dir.clone();
    let scan_id_out = scan_id.clone();
    let scope = match scope {
        ScanScopeInput::All => ScanScope::All,
        ScanScopeInput::Steam => ScanScope::Steam,
        ScanScopeInput::Directory { directory_id } => ScanScope::RomDirectory(directory_id),
        ScanScopeInput::Platform { platform_id } => ScanScope::Platform(platform_id),
    };

    tauri::async_runtime::spawn_blocking(move || {
        let report = run_scan(
            &db,
            &artwork_dir,
            scope,
            scan_id.clone(),
            cancel,
            Some(&app),
        );
        app.state::<ScanRegistry>().finish(&scan_id, report.clone());
        let _ = app.emit("scan-complete", &report);
    });
    Ok(scan_id_out)
}

/// Fetch metadata + artwork for every game missing box art, using the
/// configured provider. Optionally restricted to one platform. Reports
/// progress through the same `scan-progress` / `scan-complete` events.
#[tauri::command]
pub async fn enrich_artwork(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    platform_id: Option<String>,
) -> Result<String> {
    let provider_id = metadata::provider_statuses(&state.db)?
        .into_iter()
        .find(|p| p.configured)
        .map(|p| p.id)
        .ok_or_else(|| {
            AppError::Provider(
                "No metadata provider is configured. Add an API key in Settings → Metadata Providers \
                 to fetch artwork for ROM and manually-added games."
                    .into(),
            )
        })?;

    let scan_id = uuid::Uuid::new_v4().to_string();
    let registry = app.state::<ScanRegistry>();
    let cancel = registry.begin(&scan_id);

    let db = state.db.clone();
    let artwork_dir = state.paths.artwork_dir.clone();
    let scan_id_out = scan_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let report = crate::enrich::run_enrich(
            &db,
            &artwork_dir,
            &provider_id,
            platform_id.as_deref(),
            scan_id.clone(),
            cancel,
            Some(&app),
        );
        app.state::<ScanRegistry>().finish(&scan_id, report.clone());
        let _ = app.emit("scan-complete", &report);
    });
    Ok(scan_id_out)
}

#[tauri::command]
pub fn cancel_scan(registry: State<'_, ScanRegistry>, scan_id: String) -> bool {
    registry.cancel(&scan_id)
}

#[tauri::command]
pub fn get_last_scan_report(registry: State<'_, ScanRegistry>) -> Option<ScanReport> {
    registry.last_report.lock().unwrap().clone()
}
