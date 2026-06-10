//! Repository functions translating between SQLite rows and domain models.

pub mod artwork;
pub mod collections;
pub mod emulators;
pub mod games;
pub mod platforms;
pub mod rom_dirs;
pub mod sessions;
pub mod settings;

use chrono::Utc;

/// Current timestamp in the canonical storage format (RFC 3339, UTC).
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Deserialize a JSON-array column, tolerating malformed data.
pub fn json_vec(raw: String) -> Vec<String> {
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn json_map(raw: String) -> std::collections::HashMap<String, String> {
    serde_json::from_str(&raw).unwrap_or_default()
}
