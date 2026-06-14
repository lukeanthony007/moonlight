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

/// A TOSEC-style version token: `v1.003`, `v2`, `V1.1.2` — a `v` followed by
/// digits/dots. Roman numerals and bare letters are untouched.
fn is_version_token(token: &str) -> bool {
    let rest = match token.strip_prefix('v').or_else(|| token.strip_prefix('V')) {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
        && rest.chars().any(|c| c.is_ascii_digit())
}

fn strip_version_tokens(title: &str) -> String {
    title
        .split_whitespace()
        .filter(|t| !is_version_token(t))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clean a ROM file name into a human title. Strips the extension, bracketed
/// tags like `(USA)` / `[!]`, version tokens like `v1.003`, and normalizes
/// separators. Returns the title and a detected region when present.
pub fn title_from_filename(file_name: &str) -> (String, Option<String>) {
    let (raw, region) = title_from_filename_unversioned(file_name);
    (strip_version_tokens(&raw), region)
}

/// Extract a release date from a ROM filename's parenthesised tags. Matches a
/// TOSEC/No-Intro year `(2000)` or a full `(2000-08-06)` date, returning
/// ISO-8601 (`YYYY-MM-DD`, year-only → `YYYY-01-01`). Implausible years are
/// ignored so build numbers and IDs are not mistaken for dates.
pub fn date_from_filename(file_name: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut tag = String::new();
    let mut depth = 0u32;
    for c in file_name.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                tag.clear();
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                let t = tag.trim();
                // Full date YYYY-MM-DD.
                if t.len() == 10 && t.as_bytes()[4] == b'-' && t.as_bytes()[7] == b'-' {
                    if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
                        return Some(d.format("%Y-%m-%d").to_string());
                    }
                }
                // Bare year.
                if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(y) = t.parse::<i32>() {
                        if (1970..=2030).contains(&y) && best.is_none() {
                            best = Some(format!("{y}-01-01"));
                        }
                    }
                }
            }
            _ if depth > 0 => tag.push(c),
            _ => {}
        }
    }
    best
}

/// Like [`title_from_filename`] but keeps version tokens — the form older
/// scans produced, used to recognize stale auto-generated titles.
fn title_from_filename_unversioned(file_name: &str) -> (String, Option<String>) {
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

#[derive(Clone)]
struct RomFile {
    path: String,
    file_name: String,
    size: i64,
}

/// CD-image track extensions that belong to a `.cue`/`.gdi`/`.ccd` sheet and
/// must not be imported as their own games.
const TRACK_EXTENSIONS: &[&str] = &["bin", "img", "wav", "iso"];
/// Playlist / cue-sheet extensions whose presence in a folder means the track
/// files in that folder are part of a disc image.
const PLAYLIST_EXTENSIONS: &[&str] = &["cue", "ccd", "gdi"];

fn ext_of(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn collect_rom_files(
    dir: &RomDirectory,
    platform: &Platform,
    cancel: &AtomicBool,
) -> Result<Vec<RomFile>> {
    let extensions: Vec<String> = platform
        .extensions
        .iter()
        .map(|e| e.to_lowercase())
        .collect();

    // First pass: collect every file and note which directories contain a
    // cue/gdi/ccd sheet (so we can drop their raw track files).
    let mut all = Vec::new();
    let mut playlist_dirs: std::collections::HashSet<std::path::PathBuf> = Default::default();
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
        let path = entry.path().to_path_buf();
        if PLAYLIST_EXTENSIONS.contains(&ext_of(&path).as_str()) {
            if let Some(parent) = path.parent() {
                playlist_dirs.insert(parent.to_path_buf());
            }
        }
        all.push(path);
    }

    let mut files = Vec::new();
    for path in all {
        let ext = ext_of(&path);
        if !extensions.is_empty() && !extensions.contains(&ext) {
            continue;
        }
        // Skip raw CD tracks (`.bin`/`.img`/`.wav`/`.iso`) when a cue sheet in
        // the same folder references them — only the `.cue` is the game.
        if TRACK_EXTENSIONS.contains(&ext.as_str())
            && path.parent().is_some_and(|p| playlist_dirs.contains(p))
        {
            continue;
        }
        let size = std::fs::metadata(&path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
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

/// Disc index from a multi-disc dump name: `(Disc 1 of 4)`, `(Disk 2)`, `(CD 3)`.
fn disc_number(file_name: &str) -> Option<u32> {
    let lower = file_name.to_lowercase();
    let bytes = lower.as_bytes();
    for marker in ["disc ", "disk ", "cd "] {
        let mut start = 0;
        while let Some(pos) = lower[start..].find(marker) {
            let abs = start + pos;
            let preceded = abs == 0 || matches!(bytes[abs - 1], b'(' | b'[' | b' ');
            let digits: String = lower[abs + marker.len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if preceded && !digits.is_empty() {
                return digits.parse().ok();
            }
            start = abs + marker.len();
        }
    }
    None
}

/// Collapse multi-disc dumps: files sharing a cleaned title where at least one
/// carries a disc marker become one group booting the lowest-numbered disc.
/// Returns the reduced file list plus, per collapsed group, every member path
/// (primary first) so existing per-disc games can be reconciled.
fn collapse_disc_groups(files: Vec<RomFile>) -> (Vec<RomFile>, Vec<Vec<String>>) {
    let mut by_title: HashMap<String, Vec<(Option<u32>, RomFile)>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for file in files {
        let (title, _) = title_from_filename(&file.file_name);
        let disc = disc_number(&file.file_name);
        let key = title.to_lowercase();
        if !by_title.contains_key(&key) {
            order.push(key.clone());
        }
        by_title.entry(key).or_default().push((disc, file));
    }

    let mut kept = Vec::new();
    let mut groups = Vec::new();
    for key in order {
        let mut members = by_title.remove(&key).unwrap();
        let is_multi_disc = members.len() > 1 && members.iter().all(|(d, _)| d.is_some());
        if !is_multi_disc {
            kept.extend(members.into_iter().map(|(_, f)| f));
            continue;
        }
        members.sort_by_key(|(d, _)| d.unwrap_or(u32::MAX));
        let paths: Vec<String> = members.iter().map(|(_, f)| f.path.clone()).collect();
        kept.push(members.remove(0).1);
        groups.push(paths);
    }
    (kept, groups)
}

/// Merge any existing games for a multi-disc group into one entry whose
/// installation boots the primary (first) disc. Favorited/earliest game wins.
fn reconcile_disc_group(
    conn: &Connection,
    paths: &[String],
    emulator_id: Option<&str>,
    report: &mut ScanReport,
) -> Result<()> {
    let primary = &paths[0];
    let mut game_ids: Vec<String> = Vec::new();
    for path in paths {
        if let Some(gid) = game_id_for_source(conn, path)? {
            if !game_ids.contains(&gid) {
                game_ids.push(gid);
            }
        }
    }
    if game_ids.is_empty() {
        return Ok(()); // nothing imported yet; the scan loop creates it fresh
    }
    game_ids.sort_by_key(|id| game_sort_key(conn, id).unwrap_or((true, i64::MAX)));
    let keep = game_ids[0].clone();
    let mut merged = false;
    for gid in &game_ids[1..] {
        games::delete(conn, gid)?;
        merged = true;
    }

    let already_canonical: bool = conn
        .query_row(
            "SELECT COUNT(*) = 1 AND MAX(CASE WHEN source_id = ?2 AND installed = 1 THEN 1 ELSE 0 END) = 1
             FROM installations WHERE game_id = ?1",
            params![keep, primary],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(false);
    if already_canonical && !merged {
        return Ok(());
    }
    let size = std::fs::metadata(primary)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    conn.execute("DELETE FROM installations WHERE game_id = ?1", [&keep])?;
    conn.execute(
        "INSERT INTO installations (id, game_id, source_type, source_id, path, emulator_id, installed, file_size)
         VALUES (?1, ?2, 'rom', ?3, ?3, ?4, 1, ?5)",
        params![repo::new_id(), keep, primary, emulator_id, size],
    )?;
    report.updated += 1;
    Ok(())
}

/// Delete games whose ROM installation is a raw CD track (`.bin`/`.img`/…)
/// that a `.cue`/`.gdi`/`.ccd` in the same folder supersedes — leftovers from
/// before track files were skipped.
fn purge_superseded_track_games(
    conn: &Connection,
    dir: &RomDirectory,
    report: &mut ScanReport,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT g.id, i.path FROM games g JOIN installations i ON i.game_id = g.id
         WHERE g.platform_id = ?1 AND i.source_type = 'rom' AND i.path LIKE ?2",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![dir.platform_id, format!("{}%", dir.path)], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    for (id, path) in rows {
        let p = std::path::Path::new(&path);
        if !TRACK_EXTENSIONS.contains(&ext_of(p).as_str()) {
            continue;
        }
        let superseded = p.parent().is_some_and(|dir| {
            std::fs::read_dir(dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .any(|e| PLAYLIST_EXTENSIONS.contains(&ext_of(&e.path()).as_str()))
                })
                .unwrap_or(false)
        });
        if superseded {
            games::delete(conn, &id)?;
            report.skipped += 1;
        }
    }
    Ok(())
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

    // Switch dumps ship a base game plus its update and many DLC files, all
    // sharing one base application ID. Collapse them into one game per title.
    if dir.platform_id == "switch" {
        return sync_switch_directory(db, dir, &platform, cancel, progress, report);
    }

    progress.report("discovering", 0, 0, &format!("Scanning {}", dir.path));
    if !Path::new(&dir.path).exists() {
        report
            .errors
            .push(format!("directory not found: {}", dir.path));
        return Ok(());
    }

    // Remove games wrongly created from raw CD tracks in earlier scans (a
    // `.bin`/`.img`/`.wav` superseded by a `.cue` in the same folder).
    db.with(|c| purge_superseded_track_games(c, dir, report))?;

    // Multi-disc dumps (Shenmue Disc 1–4…) collapse to one game per title,
    // booting disc 1; existing per-disc entries are merged.
    let (files, disc_groups) = collapse_disc_groups(collect_rom_files(dir, &platform, cancel)?);
    let group_emulator = dir
        .emulator_id
        .clone()
        .or_else(|| platform.default_emulator_id.clone());
    for group in &disc_groups {
        db.with(|c| reconcile_disc_group(c, group, group_emulator.as_deref(), report))?;
    }
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
                // Known file: ensure it is flagged installed. Metadata stays
                // untouched, except stale auto-generated titles (the exact
                // string an older scan derived from this filename) are healed
                // to the current, cleaner derivation.
                let game_id: String = c.query_row(
                    "SELECT game_id FROM installations WHERE id = ?1",
                    [&install_id],
                    |r| r.get(0),
                )?;
                let current: String =
                    c.query_row("SELECT title FROM games WHERE id = ?1", [&game_id], |r| {
                        r.get(0)
                    })?;
                let (clean, _) = title_from_filename(&file.file_name);
                let (stale, _) = title_from_filename_unversioned(&file.file_name);
                let healed = if current == stale && current != clean {
                    set_title_if_unlocked(c, &game_id, &clean)?
                } else {
                    false
                };
                // Backfill a release date from the filename when the game has
                // none yet (older scans never extracted it).
                let dated = if let Some(date) = date_from_filename(&file.file_name) {
                    c.execute(
                        "UPDATE games SET release_date = ?1
                         WHERE id = ?2 AND release_date IS NULL",
                        params![date, game_id],
                    )? > 0
                } else {
                    false
                };
                let changed = c.execute(
                    "UPDATE installations SET installed = 1, file_size = ?1 WHERE id = ?2 AND installed = 0",
                    params![file.size, install_id],
                )?;
                if changed > 0 || healed || dated {
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
            let release_date = date_from_filename(&file.file_name);
            let game = games::insert(
                c,
                &games::NewGame {
                    title: &title,
                    platform_id: &dir.platform_id,
                    release_date: release_date.as_deref(),
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

/// Scan a Nintendo Switch directory, collapsing each game's base / update /
/// DLC files into a single library entry keyed by base application ID. Skips
/// homebrew (`.nro`) tools. Self-healing: pre-existing per-file duplicates for
/// the same base ID are merged into one game (favorite/edits preserved).
fn sync_switch_directory(
    db: &Db,
    dir: &RomDirectory,
    platform: &Platform,
    cancel: &AtomicBool,
    progress: &ProgressSink,
    report: &mut ScanReport,
) -> Result<()> {
    progress.report("discovering", 0, 0, &format!("Scanning {}", dir.path));
    if !Path::new(&dir.path).exists() {
        report
            .errors
            .push(format!("directory not found: {}", dir.path));
        return Ok(());
    }

    // Remove leftovers the current scanner would skip: homebrew (`.nro`) and
    // loose scene-release / empty-titled files from earlier scans.
    db.with(|c| delete_homebrew_games(c, report))?;

    let all_files = collect_rom_files(dir, platform, cancel)?;

    // Group files by base application ID — the title ID may live on the file
    // OR on an ancestor folder. Files without any title ID stay standalone;
    // `.nro` homebrew is skipped entirely.
    let mut groups: HashMap<u64, Vec<RomFile>> = HashMap::new();
    let mut standalone: Vec<RomFile> = Vec::new();
    for file in all_files {
        if file.file_name.to_lowercase().ends_with(".nro") {
            report.skipped += 1;
            continue;
        }
        match crate::switch_art::extract_title_id_from_path(&file.path) {
            Some(tid) => groups
                .entry(crate::switch_art::base_app_id(tid))
                .or_default()
                .push(file),
            None => standalone.push(file),
        }
    }

    let total = (groups.len() + standalone.len()) as u32;
    let mut index = 0u32;
    let emulator_id = dir
        .emulator_id
        .clone()
        .or_else(|| platform.default_emulator_id.clone());

    for (base, mut files) in groups {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        // Pick the bootable file: base > update > DLC, then largest.
        files.sort_by_key(|f| {
            let tid = crate::switch_art::extract_title_id_from_path(&f.path).unwrap_or(0);
            (
                crate::switch_art::title_kind_priority(tid),
                std::cmp::Reverse(f.size),
            )
        });
        let primary = files[0].clone();
        let base_hex = format!("{base:016X}");
        let group_paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        // Prefer the game-folder name over scene-release / update filenames.
        let title = best_switch_title(&files);
        index += 1;
        progress.report("importing", index, total, &title);

        db.with(|c| {
            reconcile_switch_group(
                c,
                &base_hex,
                &primary,
                &group_paths,
                &title,
                emulator_id.as_deref(),
                report,
            )
        })?;
    }

    // Standalone files (no title ID): one game per file, deduped by path.
    for file in standalone {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        index += 1;
        progress.report("importing", index, total, &file.file_name);
        db.with(|c| upsert_standalone_rom(c, dir, platform, &file, report))?;
    }

    Ok(())
}

/// Collapse all existing games tied to a base ID into one, or create it.
fn reconcile_switch_group(
    conn: &Connection,
    base_hex: &str,
    primary: &RomFile,
    group_paths: &[String],
    title: &str,
    emulator_id: Option<&str>,
    report: &mut ScanReport,
) -> Result<()> {
    // Existing games for this base: the canonical row (source_id = base_hex)
    // plus any legacy per-file rows (source_id = a path in the group).
    let mut game_ids: Vec<String> = Vec::new();
    let mut push = |id: String| {
        if !game_ids.contains(&id) {
            game_ids.push(id);
        }
    };
    if let Some(gid) = game_id_for_source(conn, base_hex)? {
        push(gid);
    }
    for path in group_paths {
        if let Some(gid) = game_id_for_source(conn, path)? {
            push(gid);
        }
    }

    if game_ids.is_empty() {
        let game = games::insert(
            conn,
            &games::NewGame {
                title,
                platform_id: "switch",
                release_date: None,
                region: None,
            },
        )?;
        conn.execute(
            "INSERT INTO installations (id, game_id, source_type, source_id, path, emulator_id, installed, file_size)
             VALUES (?1, ?2, 'rom', ?3, ?4, ?5, 1, ?6)",
            params![repo::new_id(), game.id, base_hex, primary.path, emulator_id, primary.size],
        )?;
        report.added += 1;
        return Ok(());
    }

    // Keep the favorite (or earliest-created) game; delete the rest.
    game_ids.sort_by_key(|id| game_sort_key(conn, id).unwrap_or((true, i64::MAX)));
    let keep = game_ids[0].clone();
    let mut merged = false;
    for gid in &game_ids[1..] {
        games::delete(conn, gid)?;
        merged = true;
    }

    // Replace any auto-generated title with the cleaner derived one.
    let title_changed = set_title_if_unlocked(conn, &keep, title)?;

    // Normalize the kept game to a single canonical installation. Skip the
    // rewrite when it already matches, so unchanged rescans stay quiet.
    let already_canonical: bool = conn
        .query_row(
            "SELECT COUNT(*) = 1 AND MAX(CASE WHEN source_id = ?2 AND path = ?3 AND installed = 1 THEN 1 ELSE 0 END) = 1
             FROM installations WHERE game_id = ?1",
            params![keep, base_hex, primary.path],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(false);

    if already_canonical && !merged && !title_changed {
        report.skipped += 1;
        return Ok(());
    }
    conn.execute("DELETE FROM installations WHERE game_id = ?1", [&keep])?;
    conn.execute(
        "INSERT INTO installations (id, game_id, source_type, source_id, path, emulator_id, installed, file_size)
         VALUES (?1, ?2, 'rom', ?3, ?4, ?5, 1, ?6)",
        params![repo::new_id(), keep, base_hex, primary.path, emulator_id, primary.size],
    )?;
    report.updated += 1;
    Ok(())
}

/// Update a game's title (and sort title) unless the user locked the field.
/// Returns whether a change was written.
fn set_title_if_unlocked(conn: &Connection, game_id: &str, title: &str) -> Result<bool> {
    if title.trim().is_empty() {
        return Ok(false);
    }
    let (current, locked): (String, String) = conn.query_row(
        "SELECT title, locked_fields FROM games WHERE id = ?1",
        [game_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if current == title || locked.contains("\"title\"") {
        return Ok(false);
    }
    conn.execute(
        "UPDATE games SET title = ?1, sort_title = ?2 WHERE id = ?3",
        params![title, games::sort_title_for(title), game_id],
    )?;
    Ok(true)
}

/// Delete Switch games the current scanner would no longer import: homebrew
/// (`.nro`) tools, and loose scene-release / empty-titled standalone files that
/// carry no title ID (e.g. `v-prince_of_persia_the_lost_crown.nsp` at the root)
/// — these are leftovers from earlier scans.
fn delete_homebrew_games(conn: &Connection, report: &mut ScanReport) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT g.id, i.path FROM games g JOIN installations i ON i.game_id = g.id
         WHERE g.platform_id = 'switch' AND i.source_type = 'rom'",
    )?;
    let rows: Vec<(String, Option<String>)> = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    for (id, path) in rows {
        let Some(path) = path else { continue };
        let lower = path.to_lowercase();
        let unwanted = if lower.ends_with(".nro") {
            true
        } else if crate::switch_art::extract_title_id_from_path(&path).is_none() {
            // No title ID anywhere — a loose file. Drop it only if its name is
            // scene-release junk or empty (real loose games keep clean names).
            let file_name = Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let (title, _) = title_from_filename(file_name);
            title.is_empty() || is_scene_release(&title)
        } else {
            false
        };
        if unwanted {
            games::delete(conn, &id)?;
            report.skipped += 1;
        }
    }
    Ok(())
}

/// A scene-release / bare-version name that is never a real game title
/// (`v-prince…`, `sxs-mk8u`, `v131072`). Case-sensitive on the lowercase `v-`
/// prefix so legitimate titles like `V-Rally` are not affected.
fn is_scene_release(title: &str) -> bool {
    title.starts_with("v-")
        || title.starts_with("sxs")
        || title.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Score a candidate title: more alphabetic content is better; scene-release
/// and bare-version names (`v-…`, `sxs-…`, `v131072`) are heavily penalized.
fn title_score(title: &str) -> i32 {
    let t = title.trim();
    if t.is_empty() {
        return i32::MIN;
    }
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count() as i32;
    let lower = t.to_lowercase();
    let penalized = lower.starts_with("v-")
        || lower.starts_with("v ")
        || lower.starts_with("sxs")
        || t.chars().next().is_some_and(|c| c.is_ascii_digit());
    alpha - if penalized { 1000 } else { 0 }
}

/// Best human title for a Switch game group. Considers each file's cleaned
/// name and any title-ID-bearing ancestor folder (e.g.
/// `Prince Of Persia - The Lost Crown [0100…]`), falling back to the immediate
/// parent folder when only scene-release filenames are available.
fn best_switch_title(files: &[RomFile]) -> String {
    let mut best: Option<(i32, String)> = None;
    let mut consider = |raw: &str| {
        let (title, _) = title_from_filename(raw);
        let score = title_score(&title);
        if score > 0 && best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, title));
        }
    };
    for file in files {
        consider(&file.file_name);
        // Ancestor directories that carry a title ID are the game's folder.
        let mut comps: Vec<&str> = file.path.split(['/', '\\']).collect();
        comps.pop(); // drop the filename
        for comp in comps {
            if crate::switch_art::extract_title_id(comp).is_some() {
                consider(comp);
            }
        }
    }
    if let Some((_, title)) = best {
        return title;
    }
    // Fallback: the immediate parent folder, then the cleaned filename.
    for file in files {
        if let Some(parent) = Path::new(&file.path)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
        {
            let (title, _) = title_from_filename(parent);
            if !title.trim().is_empty() {
                return title;
            }
        }
    }
    title_from_filename(&files[0].file_name).0
}

fn game_id_for_source(conn: &Connection, source_id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT game_id FROM installations WHERE source_type = 'rom' AND source_id = ?1",
            [source_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

/// (not-favorite, rowid) sort key: favorites first, then creation order.
fn game_sort_key(conn: &Connection, game_id: &str) -> Result<(bool, i64)> {
    Ok(conn.query_row(
        "SELECT favorite = 0, rowid FROM games WHERE id = ?1",
        [game_id],
        |r| Ok((r.get::<_, bool>(0)?, r.get::<_, i64>(1)?)),
    )?)
}

/// Insert/refresh a single-file ROM game (used for Switch files without a
/// title ID, e.g. loose `Game.nsp`).
fn upsert_standalone_rom(
    conn: &Connection,
    dir: &RomDirectory,
    platform: &Platform,
    file: &RomFile,
    report: &mut ScanReport,
) -> Result<()> {
    if find_installation_by_source(conn, "rom", &file.path)?.is_some() {
        report.skipped += 1;
        return Ok(());
    }
    let (title, region) = title_from_filename(&file.file_name);
    if title.is_empty() || is_scene_release(&title) {
        // Empty or scene-release junk (`v-prince…`, `sxs-…`) — usually a loose
        // copy of a properly-foldered dump. Don't create a game for it.
        report.skipped += 1;
        return Ok(());
    }
    // Avoid creating a duplicate of a game already imported under the same
    // title on this platform (e.g. a loose copy alongside a foldered dump).
    let duplicate: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM games WHERE platform_id = ?1 AND sort_title = ?2)",
        params![dir.platform_id, games::sort_title_for(&title)],
        |r| r.get(0),
    )?;
    if duplicate {
        report.skipped += 1;
        return Ok(());
    }
    let game = games::insert(
        conn,
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
    conn.execute(
        "INSERT INTO installations (id, game_id, source_type, source_id, path, emulator_id, installed, file_size)
         VALUES (?1, ?2, 'rom', ?3, ?3, ?4, 1, ?5)",
        params![repo::new_id(), game.id, file.path, emulator_id, file.size],
    )?;
    report.added += 1;
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

    fn switch_dir(db: &Db, path: &str) -> RomDirectory {
        db.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO platforms (id, name, short_name, extensions) VALUES ('switch','Nintendo Switch','Switch','[\"nsp\",\"xci\",\"nca\",\"nro\"]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        RomDirectory {
            id: "swdir".into(),
            path: path.into(),
            platform_id: "switch".into(),
            emulator_id: None,
            enabled: true,
        }
    }

    #[test]
    fn switch_collapses_base_update_and_dlc_into_one_game() {
        let tmp = tempfile::tempdir().unwrap();
        let dlc = tmp.path().join("Smash [DLC]");
        std::fs::create_dir_all(&dlc).unwrap();
        // Base, update and three DLC files for Smash (base 01006A800016E000),
        // plus a homebrew tool and a standalone game without a title ID.
        std::fs::write(
            tmp.path()
                .join("Super Smash Bros Ultimate [01006A800016E000][v0].nsp"),
            b"BASEDATA",
        )
        .unwrap();
        std::fs::write(
            tmp.path()
                .join("Super Smash Bros Ultimate [01006A800016E800][v983040].nsp"),
            b"UPD",
        )
        .unwrap();
        std::fs::write(
            dlc.join("Smash [Challenger Pack 1] [01006A800016F002].nsp"),
            b"d1",
        )
        .unwrap();
        std::fs::write(
            dlc.join("Smash [Challenger Pack 2] [01006A800016F003].nsp"),
            b"d2",
        )
        .unwrap();
        std::fs::write(
            dlc.join("Smash [Spirit Pack] [01006A800016F070].nsp"),
            b"d3",
        )
        .unwrap();
        std::fs::write(tmp.path().join("JKSV.nro"), b"homebrew").unwrap();
        std::fs::write(tmp.path().join("Super Mario Odyssey.nsp"), b"odyssey").unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = switch_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        // Two games: Smash (collapsed) + Mario Odyssey (standalone). No homebrew.
        let count: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 2, "base+update+3 DLC should collapse to one game");

        // The Smash installation points at the base file and is keyed by base id.
        let (path, source_id): (String, String) = db
            .with(|c| {
                Ok(c.query_row(
                    "SELECT i.path, i.source_id FROM installations i JOIN games g ON g.id = i.game_id
                     WHERE g.title LIKE 'Super Smash%'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?)
            })
            .unwrap();
        assert!(
            path.ends_with("01006A800016E000][v0].nsp"),
            "should boot the base file, got {path}"
        );
        assert_eq!(source_id, "01006A800016E000");
    }

    #[test]
    fn switch_rescan_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Zelda TOTK [0100F2C0115B6000][v0].xci"),
            b"totk",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("Zelda TOTK [0100F2C0115B6800][v1].nsp"),
            b"totkupd",
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = switch_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(false);

        let mut r1 = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut r1).unwrap();
        assert_eq!(r1.added, 1);

        let mut r2 = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut r2).unwrap();
        assert_eq!(r2.added, 0);
        let count: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn switch_groups_by_folder_id_and_uses_folder_title() {
        // Scene-named update/DLC files whose ID lives only on the game folder,
        // plus a clean base file at the root — all one game, titled well.
        let tmp = tempfile::tempdir().unwrap();
        let pop = tmp
            .path()
            .join("Prince Of Persia - The Lost Crown [0100210019428000]");
        let dlc = pop.join("DLC [0100210019429002]");
        std::fs::create_dir_all(&dlc).unwrap();
        std::fs::write(pop.join("v-prince_of_persia_the_lost_crown.nsp"), b"base").unwrap();
        std::fs::write(
            pop.join("v-prince_of_persia_the_lost_crown_v131072.nsp"),
            b"update",
        )
        .unwrap();
        std::fs::write(
            dlc.join("v-prince_of_persia_immortal_outfit_dlc.nsp"),
            b"dlc",
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = switch_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        let titles: Vec<String> = db
            .with(|c| {
                let mut stmt = c.prepare("SELECT title FROM games")?;
                let r = stmt
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(r)
            })
            .unwrap();
        assert_eq!(
            titles,
            vec!["Prince Of Persia - The Lost Crown"],
            "got {titles:?}"
        );
    }

    #[test]
    fn switch_skips_loose_scene_release_duplicate() {
        // A properly-foldered game plus a loose scene-named copy at the root.
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp
            .path()
            .join("Prince Of Persia - The Lost Crown [0100210019428000]");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("prince [0100210019428000][v0].nsp"), b"base").unwrap();
        let loose = tmp.path().join("v-prince_of_persia_the_lost_crown.nsp");
        std::fs::write(&loose, b"loose").unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = switch_dir(&db, tmp.path().to_str().unwrap());

        // Pre-seed the loose scene file as an earlier scan would have, to prove
        // the purge removes existing junk (not just skips new junk).
        db.with(|c| {
            let g = games::insert(
                c,
                &games::NewGame {
                    title: "v-prince of persia the lost crown",
                    platform_id: "switch",
                    release_date: None,
                    region: None,
                },
            )?;
            c.execute(
                "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                 VALUES (?1, ?2, 'rom', ?3, ?3, 1)",
                params![repo::new_id(), g.id, loose.to_str().unwrap()],
            )?;
            Ok(())
        })
        .unwrap();

        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        let titles: Vec<String> = db
            .with(|c| {
                let mut stmt = c.prepare("SELECT title FROM games")?;
                let r = stmt
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(r)
            })
            .unwrap();
        assert_eq!(
            titles,
            vec!["Prince Of Persia - The Lost Crown"],
            "loose v- copy should be skipped, got {titles:?}"
        );
    }

    #[test]
    fn switch_skips_and_removes_nro_homebrew() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("JKSV.nro"), b"hb").unwrap();
        std::fs::write(
            tmp.path().join("Kirby [01004D300C5AE000][v0].nsp"),
            b"kirby",
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = switch_dir(&db, tmp.path().to_str().unwrap());

        // Pre-seed a homebrew game as a prior scan would have.
        db.with(|c| {
            let g = games::insert(
                c,
                &games::NewGame {
                    title: "switch-time",
                    platform_id: "switch",
                    release_date: None,
                    region: None,
                },
            )?;
            c.execute(
                "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                 VALUES (?1, ?2, 'rom', ?3, ?3, 1)",
                params![repo::new_id(), g.id, "/x/switch-time.nro"],
            )?;
            Ok(())
        })
        .unwrap();

        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        let titles: Vec<String> = db
            .with(|c| {
                let mut stmt = c.prepare("SELECT title FROM games")?;
                let r = stmt
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(r)
            })
            .unwrap();
        assert_eq!(
            titles,
            vec!["Kirby"],
            "homebrew should be gone, got {titles:?}"
        );
    }

    #[test]
    fn switch_merges_existing_duplicates_and_keeps_favorite() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("Smash [01006A800016E000][v0].nsp");
        let dlc = tmp.path().join("Smash [Pack] [01006A800016F002].nsp");
        std::fs::write(&base, b"base").unwrap();
        std::fs::write(&dlc, b"dlc").unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = switch_dir(&db, tmp.path().to_str().unwrap());

        // Simulate the OLD scanner: one game per file, source_id = path. Mark
        // the DLC-derived game as the favorite to prove the merge keeps it.
        db.with(|c| {
            for (title, path, fav) in [
                ("Smash base", base.to_str().unwrap(), false),
                ("Smash dlc", dlc.to_str().unwrap(), true),
            ] {
                let g = games::insert(c, &games::NewGame { title, platform_id: "switch", release_date: None, region: None })?;
                c.execute("UPDATE games SET favorite = ?2 WHERE id = ?1", params![g.id, fav])?;
                c.execute(
                    "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                     VALUES (?1, ?2, 'rom', ?3, ?3, 1)",
                    params![repo::new_id(), g.id, path],
                )?;
            }
            Ok(())
        })
        .unwrap();

        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        // Collapsed to one game, and the favorited one survived.
        let (count, favorite): (i64, bool) = db
            .with(|c| {
                Ok(
                    c.query_row("SELECT COUNT(*), MAX(favorite) FROM games", [], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })?,
                )
            })
            .unwrap();
        assert_eq!(count, 1, "duplicates should merge into one");
        assert!(favorite, "the favorited duplicate should be the survivor");
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
    fn title_cleaning_strips_tosec_version_tokens() {
        let (title, region) =
            title_from_filename("Shenmue v1.003 (2000)(Sega)(NTSC)(US)(Disc 1 of 4)[!].gdi");
        assert_eq!(title, "Shenmue");
        assert_eq!(region.as_deref(), Some("USA"));

        let (title, _) = title_from_filename("Crazy Taxi 2 v1.004 (2001)(Sega)(NTSC)(US)[!].gdi");
        assert_eq!(title, "Crazy Taxi 2");

        // Roman numerals and bare letters are not version tokens.
        let (title, _) = title_from_filename("Grand Theft Auto V.exe");
        assert_eq!(title, "Grand Theft Auto V");
    }

    #[test]
    fn release_date_extracted_from_filename_tags() {
        assert_eq!(
            date_from_filename("Shenmue v1.003 (2000)(Sega)(NTSC)(US)(Disc 1 of 4)[!].gdi")
                .as_deref(),
            Some("2000-01-01")
        );
        assert_eq!(
            date_from_filename("Metroid (USA) (1986-08-06).nes").as_deref(),
            Some("1986-08-06")
        );
        // No year present, and implausible numbers are ignored.
        assert_eq!(date_from_filename("Super Mario 64 (USA).z64"), None);
        assert_eq!(date_from_filename("Game (9999).bin"), None);
    }

    #[test]
    fn disc_numbers_are_detected() {
        assert_eq!(
            disc_number("Shenmue v1.003 (US)(Disc 1 of 4)[!].gdi"),
            Some(1)
        );
        assert_eq!(disc_number("Final Fantasy VII (USA) (Disc 3).cue"), Some(3));
        assert_eq!(disc_number("Riven (USA) (CD 2).cue"), Some(2));
        assert_eq!(disc_number("Crazy Taxi 2 v1.004 (US)[!].gdi"), None);
    }

    #[test]
    fn multi_disc_dump_collapses_to_one_game_booting_disc_one() {
        let tmp = tempfile::tempdir().unwrap();
        for d in 1..=4 {
            let dir = tmp.path().join(format!("Shenmue (Disc {d})"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!(
                    "Shenmue v1.003 (2000)(Sega)(NTSC)(US)(Disc {d} of 4)[!].gdi"
                )),
                b"gdi",
            )
            .unwrap();
        }

        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name, extensions) VALUES ('dreamcast','Sega Dreamcast','DC','[\"gdi\",\"cdi\",\"chd\"]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let dir = RomDirectory {
            id: "dc".into(),
            path: tmp.path().to_string_lossy().to_string(),
            platform_id: "dreamcast".into(),
            emulator_id: None,
            enabled: true,
        };
        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        let (count, title, path): (i64, String, String) = db
            .with(|c| {
                Ok(c.query_row(
                    "SELECT (SELECT COUNT(*) FROM games), g.title, i.path
                     FROM games g JOIN installations i ON i.game_id = g.id",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?)
            })
            .unwrap();
        assert_eq!(count, 1, "4 discs should collapse to one game");
        assert_eq!(title, "Shenmue");
        assert!(
            path.contains("Disc 1 of 4"),
            "should boot disc 1, got {path}"
        );

        // Rescan is idempotent.
        let mut r2 = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut r2).unwrap();
        assert_eq!(r2.added, 0);
        let count: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn existing_per_disc_duplicates_merge_and_title_heals() {
        let tmp = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for d in 1..=2 {
            let dir = tmp.path().join(format!("Disc {d}"));
            std::fs::create_dir_all(&dir).unwrap();
            let p = dir.join(format!("Shenmue v1.003 (NTSC)(US)(Disc {d} of 2)[!].gdi"));
            std::fs::write(&p, b"gdi").unwrap();
            paths.push(p);
        }

        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name, extensions) VALUES ('dreamcast','Sega Dreamcast','DC','[\"gdi\"]')",
                [],
            )?;
            // Old scanner output: one game per disc, version token in title.
            for p in &paths {
                let g = games::insert(c, &games::NewGame { title: "Shenmue v1.003", platform_id: "dreamcast", release_date: None, region: None })?;
                c.execute(
                    "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                     VALUES (?1, ?2, 'rom', ?3, ?3, 1)",
                    params![repo::new_id(), g.id, p.to_str().unwrap()],
                )?;
            }
            Ok(())
        })
        .unwrap();

        let dir = RomDirectory {
            id: "dc".into(),
            path: tmp.path().to_string_lossy().to_string(),
            platform_id: "dreamcast".into(),
            emulator_id: None,
            enabled: true,
        };
        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        let (count, title): (i64, String) = db
            .with(|c| {
                Ok(c.query_row("SELECT COUNT(*), title FROM games", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?)
            })
            .unwrap();
        assert_eq!(count, 1, "per-disc duplicates should merge");
        assert_eq!(title, "Shenmue", "stale versioned title should heal");
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

    fn psx_dir(db: &Db, path: &str) -> RomDirectory {
        db.with(|c| {
            c.execute(
                "INSERT OR IGNORE INTO platforms (id, name, short_name, extensions) VALUES ('psx','PlayStation','PS1','[\"cue\",\"bin\",\"chd\"]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        RomDirectory {
            id: "psx".into(),
            path: path.into(),
            platform_id: "psx".into(),
            emulator_id: None,
            enabled: true,
        }
    }

    #[test]
    fn multi_track_cd_image_imports_as_one_game() {
        // Real-world layout: one cue sheet plus separately-named data/audio
        // tracks. Only the cue should become a game.
        let tmp = tempfile::tempdir().unwrap();
        let g = tmp.path().join("Armorines - Project S.W.A.R.M. (USA)");
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(g.join("Armorines - Project S.W.A.R.M. (USA).cue"), b"cue").unwrap();
        std::fs::write(
            g.join("Armorines - Project S.W.A.R.M. (USA) (Track 1).bin"),
            b"t1",
        )
        .unwrap();
        std::fs::write(
            g.join("Armorines - Project S.W.A.R.M. (USA) (Track 2).bin"),
            b"t2",
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = psx_dir(&db, tmp.path().to_str().unwrap());
        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        assert_eq!(report.added, 1);
        let (count, path): (i64, String) = db
            .with(|c| {
                Ok(c.query_row(
                    "SELECT (SELECT COUNT(*) FROM games), i.path FROM installations i",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?)
            })
            .unwrap();
        assert_eq!(count, 1);
        assert!(
            path.ends_with(".cue"),
            "the cue should be the installation, got {path}"
        );
    }

    #[test]
    fn purges_existing_track_duplicate_games() {
        let tmp = tempfile::tempdir().unwrap();
        let g = tmp.path().join("Castlevania - Symphony of the Night (USA)");
        std::fs::create_dir_all(&g).unwrap();
        let cue = g.join("Castlevania - Symphony of the Night (USA).cue");
        let t1 = g.join("Castlevania - Symphony of the Night (USA) (Track 1).bin");
        let t2 = g.join("Castlevania - Symphony of the Night (USA) (Track 2).bin");
        std::fs::write(&cue, b"cue").unwrap();
        std::fs::write(&t1, b"t1").unwrap();
        std::fs::write(&t2, b"t2").unwrap();

        let db = Db::open_in_memory().unwrap();
        let dir = psx_dir(&db, tmp.path().to_str().unwrap());

        // Simulate the old scanner: a game per file, tracks included.
        db.with(|c| {
            for (i, p) in [&cue, &t1, &t2].iter().enumerate() {
                let game = games::insert(c, &games::NewGame { title: &format!("Castlevania {i}"), platform_id: "psx", release_date: None, region: None })?;
                c.execute(
                    "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                     VALUES (?1, ?2, 'rom', ?3, ?3, 1)",
                    params![repo::new_id(), game.id, p.to_str().unwrap()],
                )?;
            }
            Ok(())
        })
        .unwrap();

        let cancel = AtomicBool::new(false);
        let mut report = ScanReport::default();
        sync_rom_directory(&db, &dir, &cancel, &sink(), &mut report).unwrap();

        // The two track games are purged; only the cue game remains.
        let count: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1);
        let path: String = db
            .with(|c| Ok(c.query_row("SELECT path FROM installations", [], |r| r.get(0))?))
            .unwrap();
        assert!(
            path.ends_with(".cue"),
            "only the cue should remain, got {path}"
        );
    }
}
