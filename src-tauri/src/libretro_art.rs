//! Zero-configuration ROM artwork via the public libretro-thumbnails server
//! (<https://thumbnails.libretro.com>). No API key is required.
//!
//! libretro hosts box art, title screens and in-game snaps named after the
//! No-Intro / Redump database entries, e.g.
//! `…/Nintendo - GameCube/Named_Boxarts/Animal Crossing (USA).png`.
//!
//! ROM filenames in the wild drop or reorder the region tag, so we try a small
//! set of name variants (raw file stem, cleaned title, and the title with each
//! common region suffix) and take the first that resolves to a real image.

use crate::artwork_store;
use crate::db::Db;
use crate::error::Result;
use std::path::Path;
use std::time::Duration;

/// Map a Moonlight platform id to its libretro system folder. Returns `None`
/// for platforms libretro does not cover (Steam, Switch, Wii, Arcade…).
pub fn system_for(platform_id: &str) -> Option<&'static str> {
    Some(match platform_id {
        "nes" => "Nintendo - Nintendo Entertainment System",
        "snes" => "Nintendo - Super Nintendo Entertainment System",
        "n64" => "Nintendo - Nintendo 64",
        "gamecube" => "Nintendo - GameCube",
        "gb" => "Nintendo - Game Boy",
        "gbc" => "Nintendo - Game Boy Color",
        "gba" => "Nintendo - Game Boy Advance",
        "nds" => "Nintendo - Nintendo DS",
        "psx" => "Sony - PlayStation",
        "ps2" => "Sony - PlayStation 2",
        "psp" => "Sony - PlayStation Portable",
        "genesis" => "Sega - Mega Drive - Genesis",
        "mastersystem" => "Sega - Master System - Mark III",
        "saturn" => "Sega - Saturn",
        "dreamcast" => "Sega - Dreamcast",
        _ => return None,
    })
}

/// libretro replaces these characters with `_` in thumbnail file names.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '&' | '*' | '/' | ':' | '`' | '<' | '>' | '?' | '\\' | '|' | '"' => '_',
            _ => c,
        })
        .collect()
}

/// Percent-encode a single path segment, leaving the unreserved + common
/// punctuation found in ROM names intact.
fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'('
            | b')'
            | b'\''
            | b'!'
            | b',' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

const REGION_SUFFIXES: &[&str] = &["(USA)", "(Europe)", "(Japan)", "(World)", "(USA, Europe)"];

/// Candidate display names to try for a game, most specific first.
///
/// Covers Redump/No-Intro naming including multi-disc sets (`… (Disc 1)`) and
/// the common European language tag (`(Europe) (En,Fr,De,Es)`), so TOSEC-style
/// dumps (whose raw stems never match) still resolve via the cleaned title.
pub fn name_candidates(title: &str, rom_path: Option<&str>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut push = |n: String| {
        let n = n.trim().to_string();
        if !n.is_empty() && !names.contains(&n) {
            names.push(n);
        }
    };

    if let Some(path) = rom_path {
        if let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) {
            // ".nkit"/".nkit.iso" compressed dumps embed an extra token.
            let cleaned = stem.replace(".nkit", "").replace("  ", " ");
            push(stem.to_string());
            push(cleaned);
        }
    }
    // The cleaned title, then the title qualified by each common region —
    // with multi-disc and Europe-language variants per region.
    push(title.to_string());
    for suffix in REGION_SUFFIXES {
        push(format!("{title} {suffix}"));
        push(format!("{title} {suffix} (Disc 1)"));
        if *suffix == "(Europe)" {
            push(format!("{title} (Europe) (En,Fr,De,Es)"));
            push(format!("{title} (Europe) (En,Fr,De,Es) (Disc 1)"));
        }
    }
    names
}

fn url_for(system: &str, dir: &str, name: &str) -> String {
    format!(
        "https://thumbnails.libretro.com/{}/{}/{}.png",
        encode_segment(system),
        dir,
        encode_segment(&sanitize(name)),
    )
}

/// Cheap existence check — a HEAD request that follows the No-Intro naming.
fn exists(url: &str) -> bool {
    matches!(
        ureq::head(url).timeout(Duration::from_secs(15)).call(),
        Ok(res) if res.status() == 200
    )
}

/// Attempt to fetch box art (and, when matched, the title screen as a
/// background and a snap as a screenshot) for one game. Returns `true` if box
/// art was stored. Honors user-selected artwork (never overwrites it).
pub fn fetch_for_game(
    db: &Db,
    artwork_dir: &Path,
    game_id: &str,
    system: &str,
    title: &str,
    rom_path: Option<&str>,
) -> Result<bool> {
    use crate::db::repo::artwork;

    if db
        .with(|c| artwork::has_user_selected(c, game_id, "boxart"))
        .unwrap_or(false)
    {
        return Ok(false);
    }

    // Find the first name variant that has box art, then reuse that exact name
    // for the other art kinds (they share the No-Intro name).
    let matched = name_candidates(title, rom_path)
        .into_iter()
        .find(|name| exists(&url_for(system, "Named_Boxarts", name)));

    let Some(name) = matched else {
        return Ok(false);
    };

    let boxart_url = url_for(system, "Named_Boxarts", &name);
    artwork_store::download_url(
        db,
        artwork_dir,
        game_id,
        "boxart",
        &boxart_url,
        "libretro",
        false,
    )?;

    // Best-effort secondary art; failures here don't fail the game.
    for (dir, kind) in [
        ("Named_Titles", "background"),
        ("Named_Snaps", "screenshot"),
    ] {
        let already = db
            .with(|c| artwork::has_user_selected(c, game_id, kind))
            .unwrap_or(false);
        if already {
            continue;
        }
        let url = url_for(system, dir, &name);
        if exists(&url) {
            let _ = artwork_store::download_url(
                db,
                artwork_dir,
                game_id,
                kind,
                &url,
                "libretro",
                false,
            );
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_reserved_characters() {
        assert_eq!(sanitize("Pokemon: Colosseum"), "Pokemon_ Colosseum");
        assert_eq!(sanitize("R/C Stunt Copter"), "R_C Stunt Copter");
    }

    #[test]
    fn encodes_spaces_and_keeps_parens() {
        assert_eq!(
            encode_segment("Animal Crossing (USA)"),
            "Animal%20Crossing%20(USA)"
        );
    }

    #[test]
    fn builds_expected_boxart_url() {
        let url = url_for(
            "Nintendo - GameCube",
            "Named_Boxarts",
            "Animal Crossing (USA)",
        );
        assert_eq!(
            url,
            "https://thumbnails.libretro.com/Nintendo%20-%20GameCube/Named_Boxarts/Animal%20Crossing%20(USA).png"
        );
    }

    #[test]
    fn name_candidates_cover_region_variants_and_stem() {
        let names = name_candidates("Animal Crossing", Some("/roms/Animal Crossing (USA).iso"));
        assert_eq!(names[0], "Animal Crossing (USA)"); // raw stem wins
        assert!(names.contains(&"Animal Crossing".to_string()));
        assert!(names.contains(&"Animal Crossing (Europe)".to_string()));
    }

    #[test]
    fn strips_nkit_token_from_stem() {
        let names = name_candidates("Amazing Island", Some("/roms/Amazing Island .nkit.iso"));
        assert!(names.iter().any(|n| n == "Amazing Island"));
    }

    #[test]
    fn unsupported_platforms_return_none() {
        assert!(system_for("steam").is_none());
        assert!(system_for("switch").is_none());
        assert_eq!(system_for("gamecube"), Some("Nintendo - GameCube"));
    }
}
