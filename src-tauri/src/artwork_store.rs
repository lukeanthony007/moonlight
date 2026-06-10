//! Artwork file management: importing local files, downloading URLs, and
//! storing everything inside the app data directory.

use crate::db::repo::artwork;
use crate::db::Db;
use crate::domain::Artwork;
use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico"];

fn safe_extension(raw: Option<&str>) -> &str {
    match raw {
        Some(e) if ALLOWED_EXTENSIONS.contains(&e.to_lowercase().as_str()) => e,
        _ => "png",
    }
}

fn unique_name(kind: &str, ext: &str) -> String {
    format!("{kind}-{}.{ext}", &uuid::Uuid::new_v4().to_string()[..8])
}

/// Copy a user-chosen local image into the artwork store and select it.
pub fn import_local_file(
    db: &Db,
    artwork_dir: &Path,
    game_id: &str,
    kind: &str,
    source: &str,
) -> Result<Artwork> {
    if !artwork::KINDS.contains(&kind) {
        return Err(AppError::Invalid(format!("unknown artwork kind: {kind}")));
    }
    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(AppError::NotFound(format!("file not found: {source}")));
    }
    let ext = safe_extension(source_path.extension().and_then(|e| e.to_str()));
    let dest_dir = artwork_dir.join(game_id);
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(unique_name(kind, ext));
    std::fs::copy(source_path, &dest)?;
    db.with(|c| {
        artwork::insert(
            c,
            &artwork::NewArtwork {
                game_id,
                kind,
                local_path: Some(&dest.to_string_lossy()),
                remote_url: None,
                provider: Some("user"),
                user_selected: true,
            },
        )
    })
}

/// Download an image URL into the artwork store.
pub fn download_url(
    db: &Db,
    artwork_dir: &Path,
    game_id: &str,
    kind: &str,
    url: &str,
    provider: &str,
    user_selected: bool,
) -> Result<Artwork> {
    if !artwork::KINDS.contains(&kind) {
        return Err(AppError::Invalid(format!("unknown artwork kind: {kind}")));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::Invalid("artwork URL must be http(s)".into()));
    }
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| AppError::Network(format!("failed to download artwork: {e}")))?;

    let ext_from_url = url
        .rsplit('.')
        .next()
        .filter(|e| e.len() <= 4)
        .map(|e| e.to_string());
    let ext_from_type = match response.content_type() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    };
    let ext = ext_from_type
        .map(|s| s.to_string())
        .or(ext_from_url)
        .unwrap_or_else(|| "png".to_string());
    let ext = safe_extension(Some(&ext)).to_string();

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(50 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::Network(format!("failed to read artwork: {e}")))?;
    if bytes.is_empty() {
        return Err(AppError::Network("artwork download was empty".into()));
    }

    let dest_dir = artwork_dir.join(game_id);
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(unique_name(kind, &ext));
    std::fs::write(&dest, &bytes)?;

    db.with(|c| {
        artwork::insert(
            c,
            &artwork::NewArtwork {
                game_id,
                kind,
                local_path: Some(&dest.to_string_lossy()),
                remote_url: Some(url),
                provider: Some(provider),
                user_selected,
            },
        )
    })
}

/// Delete an artwork row and its file (if stored inside our artwork dir).
pub fn delete(db: &Db, artwork_dir: &Path, artwork_id: &str) -> Result<()> {
    let local = db.with(|c| artwork::delete(c, artwork_id))?;
    if let Some(path) = local {
        let p = PathBuf::from(&path);
        if p.starts_with(artwork_dir) && p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }
    Ok(())
}

use std::io::Read;
