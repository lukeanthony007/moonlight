//! Steam library discovery. Parses Valve's text VDF format (libraryfolders.vdf
//! and appmanifest_*.acf) without requiring credentials or a running client.

use crate::db::repo::settings;
use crate::error::{AppError, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Minimal VDF value: every node is either a string or a nested map.
#[derive(Debug, Clone)]
pub enum Vdf {
    Str(String),
    Map(HashMap<String, Vdf>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(m) => m
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_map(&self) -> Option<&HashMap<String, Vdf>> {
        match self {
            Vdf::Map(m) => Some(m),
            _ => None,
        }
    }
}

/// Tokenize and parse a text VDF document into its root map.
pub fn parse_vdf(input: &str) -> Result<Vdf> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                s.push(escaped);
                            }
                        }
                        '"' => break,
                        _ => s.push(c),
                    }
                }
                tokens.push(Token::Str(s));
            }
            '{' => tokens.push(Token::Open),
            '}' => tokens.push(Token::Close),
            '/' if chars.peek() == Some(&'/') => {
                while let Some(&c) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            _ if c.is_whitespace() => {}
            _ => {
                // Unquoted token (rare in Steam files but legal VDF).
                let mut s = String::from(c);
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(Token::Str(s));
            }
        }
    }

    let mut pos = 0;
    let map = parse_map(&tokens, &mut pos)?;
    Ok(Vdf::Map(map))
}

enum Token {
    Str(String),
    Open,
    Close,
}

fn parse_map(tokens: &[Token], pos: &mut usize) -> Result<HashMap<String, Vdf>> {
    let mut map = HashMap::new();
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Close => {
                *pos += 1;
                return Ok(map);
            }
            Token::Str(key) => {
                let key = key.clone();
                *pos += 1;
                match tokens.get(*pos) {
                    Some(Token::Str(value)) => {
                        map.insert(key, Vdf::Str(value.clone()));
                        *pos += 1;
                    }
                    Some(Token::Open) => {
                        *pos += 1;
                        let inner = parse_map(tokens, pos)?;
                        map.insert(key, Vdf::Map(inner));
                    }
                    _ => return Err(AppError::Invalid("malformed VDF: key without value".into())),
                }
            }
            Token::Open => return Err(AppError::Invalid("malformed VDF: unexpected '{'".into())),
        }
    }
    Ok(map)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamApp {
    pub app_id: String,
    pub name: String,
    pub install_dir: PathBuf,
    pub library_path: PathBuf,
}

/// Candidate Steam root directories per platform.
pub fn default_steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        roots.push(home.join("Library/Application Support/Steam"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from) {
        roots.push(pf.join("Steam"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles").map(PathBuf::from) {
        roots.push(pf.join("Steam"));
    }
    roots
}

/// Locate the Steam root, honoring a user-configured override.
pub fn find_steam_root(conn: &Connection) -> Result<Option<PathBuf>> {
    if let Some(custom) = settings::get_string(conn, "steam.path")? {
        let p = PathBuf::from(&custom);
        if p.exists() {
            return Ok(Some(p));
        }
    }
    Ok(default_steam_roots()
        .into_iter()
        .find(|p| p.join("steamapps").exists()))
}

/// The command used to talk to the Steam client.
pub fn steam_command(conn: &Connection) -> Result<String> {
    if let Some(custom) = settings::get_string(conn, "steam.executable")? {
        if !custom.trim().is_empty() {
            return Ok(custom);
        }
    }
    #[cfg(target_os = "windows")]
    {
        for root in default_steam_roots() {
            let exe = root.join("steam.exe");
            if exe.exists() {
                return Ok(exe.to_string_lossy().to_string());
            }
        }
    }
    Ok("steam".to_string())
}

/// All Steam library folders reachable from the root.
pub fn library_folders(root: &Path) -> Result<Vec<PathBuf>> {
    let vdf_path = root.join("steamapps/libraryfolders.vdf");
    let mut folders = vec![root.to_path_buf()];
    if let Ok(content) = std::fs::read_to_string(&vdf_path) {
        let parsed = parse_vdf(&content)?;
        if let Some(Vdf::Map(entries)) = parsed.get("libraryfolders").cloned() {
            for value in entries.values() {
                if let Some(path) = value.get("path").and_then(|v| v.as_str()) {
                    let p = PathBuf::from(path.replace("\\\\", "\\"));
                    if p.exists() && !folders.contains(&p) {
                        folders.push(p);
                    }
                }
            }
        }
    }
    Ok(folders)
}

/// Parse one appmanifest_*.acf file.
pub fn parse_app_manifest(path: &Path, library: &Path) -> Result<Option<SteamApp>> {
    let content = std::fs::read_to_string(path)?;
    let parsed = parse_vdf(&content)?;
    let state = match parsed.get("AppState") {
        Some(s) => s,
        None => return Ok(None),
    };
    let app_id = state
        .get("appid")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let name = state
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let install_dir = state
        .get("installdir")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if app_id.is_empty() || name.is_empty() {
        return Ok(None);
    }
    Ok(Some(SteamApp {
        app_id,
        name,
        install_dir: library.join("steamapps/common").join(install_dir),
        library_path: library.to_path_buf(),
    }))
}

/// Filter for tooling/runtime entries that aren't playable games.
fn is_tool(app: &SteamApp) -> bool {
    let n = app.name.to_lowercase();
    n.contains("proton") || n.contains("steam linux runtime") || n.contains("steamworks common")
}

/// Discover all installed Steam games.
pub fn discover_installed(root: &Path) -> Result<Vec<SteamApp>> {
    let mut apps = Vec::new();
    for library in library_folders(root)? {
        let steamapps = library.join("steamapps");
        let entries = match std::fs::read_dir(&steamapps) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("appmanifest_") && name.ends_with(".acf") {
                match parse_app_manifest(&path, &library) {
                    Ok(Some(app)) if !is_tool(&app) => apps.push(app),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(file = %path.display(), error = %e, "failed to parse app manifest")
                    }
                }
            }
        }
    }
    apps.sort_by(|a, b| a.app_id.cmp(&b.app_id));
    apps.dedup_by(|a, b| a.app_id == b.app_id);
    Ok(apps)
}

/// Find existing local Steam artwork for an app in the client's library cache.
/// Returns (kind, path) pairs. Handles both flat and per-app cache layouts.
pub fn cached_artwork(root: &Path, app_id: &str) -> Vec<(String, PathBuf)> {
    let cache = root.join("appcache/librarycache");
    let mut found = Vec::new();
    let candidates: [(&str, &[&str]); 4] = [
        ("boxart", &["library_600x900.jpg", "library_600x900_2x.jpg"]),
        ("background", &["library_hero.jpg", "library_hero_2x.jpg"]),
        ("logo", &["logo.png", "logo_2x.png"]),
        ("banner", &["header.jpg"]),
    ];
    for (kind, names) in candidates {
        // Newer layout: librarycache/<appid>/<name>; older: librarycache/<appid>_<name>.
        for name in names {
            let nested = cache.join(app_id).join(name);
            if nested.exists() {
                found.push((kind.to_string(), nested));
                break;
            }
            let flat = cache.join(format!("{app_id}_{name}"));
            if flat.exists() {
                found.push((kind.to_string(), flat));
                break;
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARYFOLDERS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/user/.local/share/Steam"
		"label"		""
		"apps"
		{
			"220"		"7384338124"
		}
	}
	"1"
	{
		"path"		"/mnt/games/SteamLibrary"
		"apps"
		{
			"620"		"1244752997"
		}
	}
}
"#;

    const APPMANIFEST: &str = r#"
"AppState"
{
	"appid"		"620"
	"name"		"Portal 2"
	"StateFlags"		"4"
	"installdir"		"Portal 2"
	"SizeOnDisk"		"7384338124"
}
"#;

    #[test]
    fn parses_libraryfolders_paths() {
        let parsed = parse_vdf(LIBRARYFOLDERS).unwrap();
        let folders = parsed.get("libraryfolders").unwrap().as_map().unwrap();
        assert_eq!(folders.len(), 2);
        let p1 = folders
            .get("1")
            .unwrap()
            .get("path")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(p1, "/mnt/games/SteamLibrary");
    }

    #[test]
    fn parses_app_manifest_fields() {
        let parsed = parse_vdf(APPMANIFEST).unwrap();
        let state = parsed.get("AppState").unwrap();
        assert_eq!(state.get("appid").unwrap().as_str().unwrap(), "620");
        assert_eq!(state.get("name").unwrap().as_str().unwrap(), "Portal 2");
        assert_eq!(
            state.get("installdir").unwrap().as_str().unwrap(),
            "Portal 2"
        );
    }

    #[test]
    fn vdf_handles_comments_and_escapes() {
        let doc = "\"root\" { // comment here\n \"key\" \"value with \\\"quote\\\"\" }";
        let parsed = parse_vdf(doc).unwrap();
        let v = parsed
            .get("root")
            .unwrap()
            .get("key")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(v, "value with \"quote\"");
    }

    #[test]
    fn tool_entries_are_filtered() {
        let app = SteamApp {
            app_id: "1628350".into(),
            name: "Steam Linux Runtime 3.0 (sniper)".into(),
            install_dir: PathBuf::new(),
            library_path: PathBuf::new(),
        };
        assert!(is_tool(&app));
    }
}
