//! RetroArch executable and libretro core discovery.

use crate::catalog::core_platform_hints;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedCore {
    pub path: String,
    pub file_name: String,
    /// Human-friendly name derived from the file stem.
    pub display_name: String,
    /// Platform id this core most likely emulates, when recognizable.
    pub platform_id: Option<String>,
}

/// Directories where libretro cores are commonly installed.
pub fn default_core_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".config/retroarch/cores"));
        dirs.push(home.join(".var/app/org.libretro.RetroArch/config/retroarch/cores"));
        dirs.push(home.join("Library/Application Support/RetroArch/cores"));
    }
    dirs.push(PathBuf::from("/usr/lib/libretro"));
    dirs.push(PathBuf::from("/usr/local/lib/libretro"));
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        dirs.push(appdata.join("RetroArch/cores"));
    }
    dirs
}

fn core_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

pub fn classify_core(file_name: &str) -> Option<String> {
    let stem = file_name
        .trim_end_matches("_libretro.so")
        .trim_end_matches("_libretro.dll")
        .trim_end_matches("_libretro.dylib")
        .to_lowercase();
    core_platform_hints()
        .into_iter()
        .find(|(hint, _)| stem.starts_with(hint))
        .map(|(_, platform)| platform.to_string())
}

fn display_name(file_name: &str) -> String {
    file_name
        .trim_end_matches(&format!(".{}", core_extension()))
        .trim_end_matches("_libretro")
        .replace('_', " ")
}

/// Scan a directory (or the default locations) for libretro cores.
pub fn detect_cores(custom_dir: Option<&str>) -> Vec<DetectedCore> {
    let dirs = match custom_dir {
        Some(d) => vec![PathBuf::from(d)],
        None => default_core_dirs(),
    };
    let ext = core_extension();
    let mut cores = Vec::new();
    for dir in dirs {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            cores.push(DetectedCore {
                path: path.to_string_lossy().to_string(),
                display_name: display_name(&file_name),
                platform_id: classify_core(&file_name),
                file_name,
            });
        }
    }
    cores.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    cores.dedup_by(|a, b| a.file_name == b.file_name);
    cores
}

/// Likely RetroArch executable locations for auto-detection.
pub fn detect_executable() -> Option<String> {
    let candidates = [
        "/usr/bin/retroarch",
        "/usr/local/bin/retroarch",
        "/var/lib/flatpak/exports/bin/org.libretro.RetroArch",
        "/Applications/RetroArch.app/Contents/MacOS/RetroArch",
        "C:\\RetroArch-Win64\\retroarch.exe",
    ];
    candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_cores() {
        assert_eq!(classify_core("snes9x_libretro.so").as_deref(), Some("snes"));
        assert_eq!(
            classify_core("mupen64plus_next_libretro.so").as_deref(),
            Some("n64")
        );
        assert_eq!(classify_core("totally_unknown_libretro.so"), None);
    }
}
