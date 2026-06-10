//! Domain models shared between the database layer and the Tauri command
//! surface. All models serialize as camelCase for the TypeScript frontend.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub title: String,
    pub sort_title: String,
    pub description: Option<String>,
    /// ISO-8601 date (YYYY-MM-DD). Year-only values are stored as YYYY-01-01.
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Vec<String>,
    pub region: Option<String>,
    pub platform_id: String,
    pub series: Option<String>,
    pub favorite: bool,
    pub hidden: bool,
    pub date_added: String,
    pub last_played: Option<String>,
    pub playtime_seconds: i64,
    pub user_rating: Option<i64>,
    /// Metadata fields the user has locked against automatic updates.
    pub locked_fields: Vec<String>,
    /// Snapshot of the last provider-fetched metadata, used for "restore".
    pub provider_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub id: String,
    pub game_id: String,
    /// "steam" | "rom" | "manual"
    pub source_type: String,
    /// Stable identity within the source (Steam appid, canonical ROM path…).
    pub source_id: Option<String>,
    pub path: Option<String>,
    pub emulator_id: Option<String>,
    pub launch_args: Option<String>,
    pub working_directory: Option<String>,
    pub installed: bool,
    /// File size captured at scan time; used to reconnect moved files.
    pub file_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub manufacturer: Option<String>,
    pub default_emulator_id: Option<String>,
    pub extensions: Vec<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Emulator {
    pub id: String,
    pub name: String,
    pub executable_path: String,
    /// "retroarch" | "standalone" | "custom"
    pub emulator_type: String,
    pub platforms: Vec<String>,
    pub command_template: String,
    pub core_name: Option<String>,
    pub working_directory: Option<String>,
    pub environment: std::collections::HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    pub id: String,
    pub game_id: String,
    /// "boxart" | "background" | "screenshot" | "logo" | "icon" | "banner"
    pub kind: String,
    pub local_path: Option<String>,
    pub remote_url: Option<String>,
    pub provider: Option<String>,
    pub user_selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySession {
    pub id: String,
    pub game_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<i64>,
    /// "ok" | "error(<code>)" | "detached" | "running"
    pub exit_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub sort_order: i64,
    pub game_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RomDirectory {
    pub id: String,
    pub path: String,
    pub platform_id: String,
    pub emulator_id: Option<String>,
    pub enabled: bool,
}

/// A game joined with its installations and resolved artwork, as consumed by
/// the library views.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    #[serde(flatten)]
    pub game: Game,
    pub installations: Vec<Installation>,
    /// kind -> local path of the effective artwork for that slot.
    pub artwork: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub scan_id: String,
    pub source: String,
    pub added: u32,
    pub updated: u32,
    pub missing: u32,
    pub reconnected: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub duration_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: String,
    pub source: String,
    pub phase: String,
    pub current: u32,
    pub total: u32,
    pub message: String,
}

/// Patch payload for user-editable game metadata. `None` = leave unchanged.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePatch {
    pub title: Option<String>,
    pub sort_title: Option<String>,
    pub description: Option<Option<String>>,
    pub release_date: Option<Option<String>>,
    pub developer: Option<Option<String>>,
    pub publisher: Option<Option<String>>,
    pub genres: Option<Vec<String>>,
    pub region: Option<Option<String>>,
    pub series: Option<Option<String>>,
    pub user_rating: Option<Option<i64>>,
    pub locked_fields: Option<Vec<String>>,
}
