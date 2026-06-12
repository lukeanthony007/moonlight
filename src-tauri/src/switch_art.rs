//! Zero-configuration Nintendo Switch artwork.
//!
//! libretro-thumbnails has no Switch set, but Switch dumps embed the 16-hex
//! title ID in the filename (e.g. `Zelda TOTK [0100F2C0115B6000].xci`). The
//! community `tinfoil.media` CDN serves the official eShop icon and banner for
//! any base application ID, with no API key:
//!
//! - icon:   `https://api.nlib.cc/nx/{base}/icon/512/512`
//! - banner: `https://api.nlib.cc/nx/{base}/banner/1280/720`
//!
//! (with `tinfoil.media` as a fallback). Update and DLC IDs have no art of
//! their own, so we derive the base application ID before looking up.

use crate::artwork_store;
use crate::db::Db;
use crate::error::Result;

/// Extract the first 16-hex-digit title ID from a ROM filename, if present.
pub fn extract_title_id(name: &str) -> Option<u64> {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_hexdigit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                i += 1;
            }
            // Title IDs are exactly 16 hex digits; ignore shorter/longer runs.
            if i - start == 16 {
                if let Ok(id) = u64::from_str_radix(&name[start..i], 16) {
                    // Switch title IDs sit in the 0x0100.. application range.
                    if id >> 32 == 0x0100 || id >> 48 == 0x0100 {
                        return Some(id);
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Candidate base application IDs (uppercase hex) for an arbitrary title ID.
/// Handles base titles, updates (`…800`) and DLC (`base + 0x1000 + n`).
pub fn base_candidates(title_id: u64) -> Vec<String> {
    let mut out = Vec::new();
    let masked = title_id & !0xFFF; // base + update
    let dlc_base = title_id.wrapping_sub(0x1000) & !0xFFF; // DLC → owning app
    for id in [masked, dlc_base] {
        let hex = format!("{id:016X}");
        if !out.contains(&hex) {
            out.push(hex);
        }
    }
    out
}

/// Icon (square box art) URL candidates for a base id, primary source first.
fn icon_urls(base: &str) -> [String; 2] {
    [
        format!("https://api.nlib.cc/nx/{base}/icon/512/512"),
        format!("https://tinfoil.media/ti/{base}/512/512/"),
    ]
}

/// Banner (wide hero) URL candidates for a base id.
fn banner_urls(base: &str) -> [String; 2] {
    [
        format!("https://api.nlib.cc/nx/{base}/banner/1280/720"),
        format!("https://tinfoil.media/thi/{base}/1280/720/"),
    ]
}

/// Whether a game has a usable Switch title ID for artwork lookup.
pub fn can_match(rom_path: Option<&str>) -> bool {
    rom_path.and_then(extract_title_id).is_some()
}

/// Try downloading the first URL that yields an image, storing it under `kind`.
/// Returns true on success. A miss (404 etc.) just moves to the next URL —
/// there is no separate HEAD probe, which keeps this to one request per hit.
fn try_download(
    db: &Db,
    artwork_dir: &std::path::Path,
    game_id: &str,
    kind: &str,
    urls: &[String],
) -> bool {
    for url in urls {
        match artwork_store::download_url(db, artwork_dir, game_id, kind, url, "nintendo", false) {
            Ok(_) => return true,
            Err(e) => tracing::debug!(kind, %url, error = %e, "switch art candidate missed"),
        }
    }
    false
}

/// Fetch the eShop icon (as box art) and banner (as background) for a Switch
/// game. Returns `true` if box art was stored. Honors user-selected artwork.
pub fn fetch_for_game(
    db: &Db,
    artwork_dir: &std::path::Path,
    game_id: &str,
    rom_path: Option<&str>,
) -> Result<bool> {
    use crate::db::repo::artwork;

    let Some(title_id) = rom_path.and_then(extract_title_id) else {
        return Ok(false);
    };
    if db
        .with(|c| artwork::has_user_selected(c, game_id, "boxart"))
        .unwrap_or(false)
    {
        return Ok(false);
    }

    // Try each base-id candidate; the first that yields an icon wins, and we
    // reuse it for the banner.
    for base in base_candidates(title_id) {
        if try_download(db, artwork_dir, game_id, "boxart", &icon_urls(&base)) {
            let has_bg = db
                .with(|c| artwork::has_user_selected(c, game_id, "background"))
                .unwrap_or(false);
            if !has_bg {
                try_download(db, artwork_dir, game_id, "background", &banner_urls(&base));
            }
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bracketed_title_id() {
        assert_eq!(
            extract_title_id("TOTK [0100F2C0115B6000][v0].xci"),
            Some(0x0100F2C0115B6000)
        );
        assert_eq!(
            extract_title_id("Animal Crossing New Horizons [01006F8002326000][v0].nsp"),
            Some(0x01006F8002326000)
        );
    }

    #[test]
    fn ignores_files_without_a_title_id() {
        assert_eq!(extract_title_id("Super Mario Odyssey.nsp"), None);
        assert_eq!(extract_title_id("Pokemon Legends Arceus.xci"), None);
        // 16 hex but not an application id (v-tags, hashes) are rejected.
        assert_eq!(extract_title_id("game [v1234567890123456].nsp"), None);
    }

    #[test]
    fn base_id_from_update_is_the_base() {
        // Update (…6800) → base …6000.
        let bases = base_candidates(0x01006F8002326800);
        assert_eq!(bases[0], "01006F8002326000");
    }

    #[test]
    fn base_id_from_dlc_resolves_to_owning_app() {
        // DLC 01006F800232712D → owning app 01006F8002326000.
        let bases = base_candidates(0x01006F800232712D);
        assert!(bases.contains(&"01006F8002326000".to_string()));
    }

    /// Live tinfoil.media check — downloads the eShop icon for a real title ID.
    /// Network-dependent; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn downloads_real_switch_icon() {
        use crate::db::repo::{games, new_id};
        use rusqlite::params;
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name) VALUES ('switch','Nintendo Switch','Switch')",
                [],
            )?;
            let game = games::insert(
                c,
                &games::NewGame { title: "Zelda TOTK", platform_id: "switch", release_date: None, region: None },
            )?;
            c.execute(
                "INSERT INTO installations (id, game_id, source_type, path, installed)
                 VALUES (?1, ?2, 'rom', '/roms/TOTK [0100F2C0115B6000][v0].xci', 1)",
                params![new_id(), game.id],
            )?;
            let dir = tempfile::tempdir().unwrap();
            let got = fetch_for_game(&db, dir.path(), &game.id, Some("/roms/TOTK [0100F2C0115B6000][v0].xci"))?;
            assert!(got, "expected box art to download");
            let path: String = c.query_row(
                "SELECT local_path FROM artwork WHERE kind='boxart' AND provider='nintendo'",
                [],
                |r| r.get(0),
            )?;
            assert!(std::fs::metadata(&path).unwrap().len() > 1000);
            Ok(())
        })
        .unwrap();
    }
}
