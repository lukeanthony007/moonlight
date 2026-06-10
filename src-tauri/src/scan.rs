//! Idempotent library scanning and synchronization.
//!
//! Identity rules:
//! - ROM installations: `(source_type='rom', source_id=<canonical path>)`.
//! - Steam installations: `(source_type='steam', source_id=<appid>)`.
//!
//! Re-scans never duplicate games, never touch user-edited metadata, and
//! reconnect moved files by matching file name + size.

use crate::db::repo::{self, artwork, games, platforms, rom_dirs};
use crate::db::Db;
use crate::domain::{Platform, RomDirectory, ScanProgress, ScanReport};
use crate::error::{AppError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use walkdir::WalkDir;

/// Registry of in-flight scans, used for cancellation.
#[derive(Default)]
pub struct ScanRegistry {
    pub active: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub last_report: Mutex<Option<ScanReport>>,
}

impl ScanRegistry {
    pub fn begin(&self, scan_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.active
            .lock()
            .unwrap()
            .insert(scan_id.to_string(), flag.clone());
        flag
    }
    pub fn finish(&self, scan_id: &str, report: ScanReport) {
        self.active.lock().unwrap().remove(scan_id);
        *self.last_report.lock().unwrap() = Some(report);
    }
    pub fn cancel(&self, scan_id: &str) -> bool {
        match self.active.lock().unwrap().get(scan_id) {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }
}

pub struct ProgressSink<'a> {
    pub app: Option<&'a tauri::AppHandle>,
    pub scan_id: String,
    pub source: String,
}

impl ProgressSink<'_> {
    pub fn report(&self, phase: &str, current: u32, total: u32, message: &str) {
        if let Some(app) = self.app {
            let _ = app.emit(
                "scan-progress",
                ScanProgress {
                    scan_id: self.scan_id.clone(),
                    source: self.source.clone(),
                    phase: phase.to_string(),
                    current,
                    total,
                    message: message.to_string(),
                },
            );
        }
    }
}

/// Clean a ROM file name into a human title. Strips the extension, bracketed
/// tags like `(USA)` / `[!]`, and normalizes separators. Returns the title
/// and a detected region when present.
pub fn title_from_filename(file_name: &str) -> (String, Option<String>) {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let mut region = None;
    let mut title = String::new();
    let mut depth = 0u32;
    let mut tag = String::new();
    for c in stem.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                tag.clear();
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                if region.is_none() {
                    let t = tag.trim();
                    for known in [
                        "USA", "Europe", "Japan", "World", "U", "E", "J", "EUR", "JPN", "NTSC",
                        "PAL",
                    ] {
                        if t.eq_ignore_ascii_case(known)
                            || t.split(',').any(|p| p.trim().eq_ignore_ascii_case(known))
                        {
                            region = Some(normalize_region(t));
                            break;
                        }
                    }
                }
            }
            _ if depth > 0 => tag.push(c),
            _ => title.push(c),
        }
    }
    let title = title.replace('_', " ");
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    // "Legend of Zelda, The" -> "The Legend of Zelda"
    let title = match title.rsplit_once(", The") {
        Some((head, rest)) if rest.trim().is_empty() => format!("The {head}"),
        _ => title,
    };
    (title.trim().to_string(), region)
}

fn normalize_region(tag: &str) -> String {
    let first = tag.split(',').next().unwrap_or(tag).trim();
    match first.to_uppercase().as_str() {
        "U" | "USA" | "NTSC" => "USA".to_string(),
        "E" | "EUR" | "EUROPE" | "PAL" => "Europe".to_string(),
        "J" | "JPN" | "JAPAN" => "Japan".to_string(),
        "WORLD" => "World".to_string(),
        _ => first.to_string(),
    }
}

struct RomFile {
    path: String,
    file_name: String,
    size: i64,
}

fn collect_rom_files(
    dir: &RomDirectory,
    platform: &Platform,
    cancel: &AtomicBool,
) -> Result<Vec<RomFile>> {
    let mut files = Vec::new();
    let extensions: Vec<String> = platform
        .extensions
        .iter()
        .map(|e| e.to_lowercase())
        .collect();
    for entry in WalkDir::new(&dir.path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !extensions.is_empty() && !extensions.contains(&ext) {
            continue;
        }
        // Multi-track formats: skip .bin files when a sibling .cue exists.
        if ext == "bin" && path.with_extension("cue").exists() {
            continue;
        }
        let size = entry.metadata().map(|m| m.len() as i64).unwrap_or(0);
        files.push(RomFile {
            path: path.to_string_lossy().to_string(),
            file_name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            size,
        });
    }
    Ok(files)
}

fn find_installation_by_source(
    conn: &Connection,
    source_type: &str,
    source_id: &str,
) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM installations WHERE source_type = ?1 AND source_id = ?2",
            params![source_type, source_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

/// Find a missing installation matching by file name + size (moved file).
fn find_moved_candidate(
    conn: &Connection,
    file_name: &str,
    size: i64,
    platform_id: &str,
) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT i.id FROM installations i
             JOIN games g ON g.id = i.game_id
             WHERE i.source_type = 'rom' AND i.file_size = ?1 AND g.platform_id = ?2
               AND i.path IS NOT NULL AND i.path LIKE ?3 AND i.installed = 0",
            params![size, platform_id, format!("%{file_name}")],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

/// Scan one ROM directory and synchronize it into the library.
pub fn sync_rom_directory(
    db: &Db,
    dir: &RomDirectory,
    cancel: &AtomicBool,
    progress: &ProgressSink,
    report: &mut ScanReport,
) -> Result<()> {
    let platform = db
        .with(|c| platforms::get(c, &dir.platform_id))?
        .ok_or_else(|| AppError::NotFound(format!("platform {} not found", dir.platform_id)))?;

    progress.report("discovering", 0, 0, &format!("Scanning {}", dir.path));
    if !Path::new(&dir.path).exists() {
        report
            .errors
            .push(format!("directory not found: {}", dir.path));
        return Ok(());
    }
    let files = collect_rom_files(dir, &platform, cancel)?;
    let total = files.len() as u32;

    // Pass 1: mark installations under this directory whose file disappeared.
    db.with(|c| {
        let mut stmt = c.prepare(
            "SELECT id, path FROM installations
             WHERE source_type = 'rom' AND installed = 1 AND path LIKE ?1",
        )?;
        let rows = stmt
            .query_map([format!("{}%", dir.path)], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, path) in rows {
            if !Path::new(&path).exists() {
                c.execute(
                    "UPDATE installations SET installed = 0 WHERE id = ?1",
                    [&id],
                )?;
                report.missing += 1;
            }
        }
        Ok(())
    })?;

    // Pass 2: upsert found files.
    for (index, file) in files.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        progress.report("importing", index as u32 + 1, total, &file.file_name);
        db.with(|c| {
            if let Some(install_id) = find_installation_by_source(c, "rom", &file.path)? {
                // Known file: ensure it is flagged installed; never touch metadata.
                let changed = c.execute(
                    "UPDATE installations SET installed = 1, file_size = ?1 WHERE id = ?2 AND installed = 0",
                    params![file.size, install_id],
                )?;
                if changed > 0 {
                    report.updated += 1;
                } else {
                    report.skipped += 1;
                }
                return Ok(());
            }
            if let Some(install_id) = find_moved_candidate(c, &file.file_name, file.size, &dir.platform_id)? {
                // Same name+size as a missing install: reconnect to new path.
                c.execute(
                    "UPDATE installations SET path = ?1, source_id = ?1, installed = 1 WHERE id = ?2",
                    params![file.path, install_id],
                )?;
                report.reconnected += 1;
                return Ok(());
            }
            let (title, region) = title_from_filename(&file.file_name);
            if title.is_empty() {
                report.skipped += 1;
                return Ok(());
            }
            let game = games::insert(
                c,
                &games::NewGame {
                    title: &title,
                    platform_id: &dir.platform_id,
                    release_date: None,
                    region: region.as_deref(),
                },
            )?;
            let emulator_id = dir
                .emulator_id
                .clone()
                .or_else(|| platform.default_emulator_id.clone());
            c.execute(
                "INSERT INTO installations (id, game_id, source_type, source_id, path, emulator_id, installed, file_size)
                 VALUES (?1, ?2, 'rom', ?3, ?3, ?4, 1, ?5)",
                params![repo::new_id(), game.id, file.path, emulator_id, file.size],
            )?;
            report.added += 1;
            Ok(())
        })?;
    }
    Ok(())
}

/// Import installed Steam games and their cached artwork.
pub fn sync_steam(
    db: &Db,
    artwork_dir: &Path,
    cancel: &AtomicBool,
    progress: &ProgressSink,
    report: &mut ScanReport,
) -> Result<()> {
    let root = db.with(crate::steam::find_steam_root)?;
    let root = match root {
        Some(r) => r,
        None => {
            report.errors.push(
                "Steam installation not found. Set a custom path in Settings → Game Libraries."
                    .into(),
            );
            return Ok(());
        }
    };
    progress.report("discovering", 0, 0, "Reading Steam libraries");
    let apps = crate::steam::discover_installed(&root)?;
    let total = apps.len() as u32;

    // Mark Steam games no longer present as uninstalled.
    db.with(|c| {
        let mut stmt = c.prepare(
            "SELECT id, source_id FROM installations WHERE source_type = 'steam' AND installed = 1",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, source_id) in rows {
            let still_installed = source_id
                .as_deref()
                .map(|sid| apps.iter().any(|a| a.app_id == sid))
                .unwrap_or(false);
            if !still_installed {
                c.execute(
                    "UPDATE installations SET installed = 0 WHERE id = ?1",
                    [&id],
                )?;
                report.missing += 1;
            }
        }
        Ok(())
    })?;

    for (index, app) in apps.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        progress.report("importing", index as u32 + 1, total, &app.name);
        let game_id = db.with(|c| {
            if let Some(install_id) = find_installation_by_source(c, "steam", &app.app_id)? {
                c.execute(
                    "UPDATE installations SET installed = 1, path = ?1 WHERE id = ?2",
                    params![app.install_dir.to_string_lossy().to_string(), install_id],
                )?;
                report.skipped += 1;
                let game_id: String = c.query_row(
                    "SELECT game_id FROM installations WHERE id = ?1",
                    [&install_id],
                    |r| r.get(0),
                )?;
                Ok(game_id)
            } else {
                let game = games::insert(
                    c,
                    &games::NewGame {
                        title: &app.name,
                        platform_id: "steam",
                        release_date: None,
                        region: None,
                    },
                )?;
                c.execute(
                    "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                     VALUES (?1, ?2, 'steam', ?3, ?4, 1)",
                    params![
                        repo::new_id(),
                        game.id,
                        app.app_id,
                        app.install_dir.to_string_lossy().to_string()
                    ],
                )?;
                report.added += 1;
                Ok(game.id)
            }
        })?;

        // Import cached Steam artwork once per kind, never displacing a
        // user-selected image.
        for (kind, source_path) in crate::steam::cached_artwork(&root, &app.app_id) {
            let result: Result<()> = (|| {
                let already_has: bool = db.with(|c| {
                    let count: i64 = c.query_row(
                        "SELECT COUNT(*) FROM artwork WHERE game_id = ?1 AND kind = ?2",
                        params![game_id, kind],
                        |r| r.get(0),
                    )?;
                    Ok(count > 0)
                })?;
                if already_has {
                    return Ok(());
                }
                let ext = source_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("jpg");
                let dest_dir = artwork_dir.join(&game_id);
                std::fs::create_dir_all(&dest_dir)?;
                let dest = dest_dir.join(format!("{kind}-steam.{ext}"));
                std::fs::copy(&source_path, &dest)?;
                db.with(|c| {
                    artwork::insert(
                        c,
                        &artwork::NewArtwork {
                            game_id: &game_id,
                            kind: &kind,
                            local_path: Some(&dest.to_string_lossy()),
                            remote_url: None,
                            provider: Some("steam"),
                            user_selected: false,
                        },
                    )?;
                    Ok(())
                })?;
                Ok(())
            })();
            if let Err(e) = result {
                tracing::warn!(app = %app.app_id, error = %e, "failed to import steam artwork");
            }
        }
    }
    Ok(())
}

/// Run a full scan over the requested scope.
pub enum ScanScope {
    All,
    Steam,
    RomDirectory(String),
    Platform(String),
}

pub fn run_scan(
    db: &Db,
    artwork_dir: &Path,
    scope: ScanScope,
    scan_id: String,
    cancel: Arc<AtomicBool>,
    app: Option<&tauri::AppHandle>,
) -> ScanReport {
    let started = std::time::Instant::now();
    let source_label = match &scope {
        ScanScope::All => "all".to_string(),
        ScanScope::Steam => "steam".to_string(),
        ScanScope::RomDirectory(id) => format!("directory:{id}"),
        ScanScope::Platform(id) => format!("platform:{id}"),
    };
    let progress = ProgressSink {
        app,
        scan_id: scan_id.clone(),
        source: source_label.clone(),
    };
    let mut report = ScanReport {
        scan_id: scan_id.clone(),
        source: source_label,
        ..Default::default()
    };

    let result: Result<()> = (|| {
        let dirs = db.with(rom_dirs::list)?;
        match &scope {
            ScanScope::All => {
                sync_steam(db, artwork_dir, &cancel, &progress, &mut report)?;
                for dir in dirs.iter().filter(|d| d.enabled) {
                    sync_rom_directory(db, dir, &cancel, &progress, &mut report)?;
                }
            }
            ScanScope::Steam => sync_steam(db, artwork_dir, &cancel, &progress, &mut report)?,
            ScanScope::RomDirectory(id) => {
                let dir = db.with(|c| rom_dirs::get(c, id))?;
                sync_rom_directory(db, &dir, &cancel, &progress, &mut report)?;
            }
            ScanScope::Platform(platform_id) => {
                if platform_id == "steam" {
                    sync_steam(db, artwork_dir, &cancel, &progress, &mut report)?;
                }
                for dir in dirs
                    .iter()
                    .filter(|d| d.enabled && &d.platform_id == platform_id)
                {
                    sync_rom_directory(db, dir, &cancel, &progress, &mut report)?;
                }
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => {}
        Err(AppError::Cancelled) => report.cancelled = true,
        Err(e) => report.errors.push(e.to_string()),
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    tracing::info!(
        scan = %scan_id,
        added = report.added,
        missing = report.missing,
        reconnected = report.reconnected,
        errors = report.errors.len(),
        "scan finished"
    );
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::sync::atomic::AtomicBool;

    fn test_dir(db: &Db, path: &str) -> RomDirectory {
        db.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO platforms (id, name, short_name, extensions) VALUES ('n64','Nintendo 64','N64','[\"z64\",\"n64\"]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        RomDirectory {
            id: "dir1".into(),
            path: path.into(),
            platform_id: "n64".into(),
            emulator_id: None,
            enabled: true,
        }
    }

    fn sink() -> ProgressSink<'static> {
        ProgressSink {
            app: None,
            scan_id: "test".into(),
            source: "test".into(),
        }
    }

    #[test]
    fn title_cleaning_handles_tags_and_articles() {
        let (title, region) = title_from_filename("Legend of Zelda, The (USA) [!].z64");
        assert_eq!(title, "The Legend of Zelda");
        assert_eq!(region.as_deref(), Some("USA"));

        let (title, region) = title_from_filename("Super_Mario_64 (Europe) (Rev A).z64");
        assert_eq!(title, "Super Mario 64");
        assert_eq!(region.as_deref(), Some("Europe"));

        let (title, _) = title_from_filename("Banjo-Kazooie.z64");
        assert_eq!(title, "Banjo-Kazooie");
    }

    #[test]
    fn rescan_does_not_duplicate_games() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Super Mario 64 (USA).z64"), b"rom-data").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), b"not a rom").unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = test_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(false);

        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();
        assert_eq!(report.added, 1);

        let mut report2 = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report2).unwrap();
        assert_eq!(report2.added, 0);
        assert_eq!(report2.skipped, 1);

        let count: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn missing_files_are_marked_and_moved_files_reconnect() {
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("Super Mario 64 (USA).z64");
        std::fs::write(&original, b"rom-data").unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = test_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(false);

        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();
        assert_eq!(report.added, 1);

        // Edit metadata as the user would.
        db.with(|c| {
            c.execute(
                "UPDATE games SET description = 'my notes', favorite = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        // Move the file to a subdirectory.
        let subdir = tmp.path().join("usa");
        std::fs::create_dir_all(&subdir).unwrap();
        let moved = subdir.join("Super Mario 64 (USA).z64");
        std::fs::rename(&original, &moved).unwrap();

        // First scan marks it missing, then reconnects to the new path.
        let mut report2 = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report2).unwrap();
        assert_eq!(report2.missing, 1);
        assert_eq!(report2.reconnected, 1);
        assert_eq!(report2.added, 0);

        let (count, path, installed): (i64, String, bool) = db
            .with(|c| {
                Ok(c.query_row(
                    "SELECT (SELECT COUNT(*) FROM games), path, installed FROM installations",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?)
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(path, moved.to_string_lossy());
        assert!(installed);

        // User metadata survived.
        let (desc, fav): (String, bool) = db
            .with(|c| {
                Ok(
                    c.query_row("SELECT description, favorite FROM games", [], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })?,
                )
            })
            .unwrap();
        assert_eq!(desc, "my notes");
        assert!(fav);
    }

    #[test]
    fn cancellation_stops_the_scan() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.z64"), b"x").unwrap();
        let db = Db::open_in_memory().unwrap();
        let dir = test_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(true);
        let mut report = ScanReport::default();
        let err = sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap_err();
        assert!(matches!(err, AppError::Cancelled));
    }

    #[test]
    fn bin_files_with_cue_sibling_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Game.cue"), b"cue").unwrap();
        std::fs::write(tmp.path().join("Game.bin"), b"bin").unwrap();

        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name, extensions) VALUES ('psx','PlayStation','PS1','[\"cue\",\"bin\"]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let dir = RomDirectory {
            id: "d".into(),
            path: tmp.path().to_string_lossy().to_string(),
            platform_id: "psx".into(),
            emulator_id: None,
            enabled: true,
        };
        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();
        assert_eq!(report.added, 1);
    }
}
