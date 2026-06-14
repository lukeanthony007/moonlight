//! LaunchBox Games Database — a keyless, account-free retro metadata source
//! (the same database the LaunchBox frontend uses).
//!
//! The official `Metadata.zip` (~100 MB) contains a ~500 MB `Metadata.xml` of
//! `<Game>` entries. We download it, stream-parse it without loading it into
//! memory, and cache the games for our platforms into the `launchbox_games`
//! table. Enrichment then matches each ROM game by platform + normalized name.

use crate::db::Db;
use crate::error::{AppError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const METADATA_URL: &str = "https://gamesdb.launchbox-app.com/Metadata.zip";

/// Map a LaunchBox platform name to a Moonlight platform id. Only mapped
/// platforms are imported, keeping the local cache focused and small.
pub fn platform_for(launchbox_name: &str) -> Option<&'static str> {
    Some(match launchbox_name {
        "Nintendo Entertainment System" => "nes",
        "Super Nintendo Entertainment System" => "snes",
        "Nintendo 64" => "n64",
        "Nintendo GameCube" => "gamecube",
        "Nintendo Wii" => "wii",
        "Nintendo Switch" => "switch",
        "Nintendo Game Boy" => "gb",
        "Nintendo Game Boy Color" => "gbc",
        "Nintendo Game Boy Advance" => "gba",
        "Nintendo DS" => "nds",
        "Sony Playstation" => "psx",
        "Sony Playstation 2" => "ps2",
        "Sony PSP" => "psp",
        "Sega Genesis" => "genesis",
        "Sega Master System" => "mastersystem",
        "Sega Saturn" => "saturn",
        "Sega Dreamcast" => "dreamcast",
        "Arcade" => "arcade",
        _ => return None,
    })
}

/// Normalized key for fuzzy name matching: lowercase, articles dropped, every
/// non-alphanumeric character removed. `The Legend of Zelda: A Link to the
/// Past` and `Legend of Zelda - A Link to the Past` both collapse to the same
/// key.
pub fn match_key(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let without_article = lower
        .strip_prefix("the ")
        .or_else(|| lower.strip_prefix("a "))
        .or_else(|| lower.strip_prefix("an "))
        .unwrap_or(&lower);
    without_article
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct LaunchboxMeta {
    pub name: String,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Option<Vec<String>>,
}

/// Look up cached metadata for a game by platform and title.
pub fn lookup(
    conn: &rusqlite::Connection,
    platform_id: &str,
    title: &str,
) -> Result<Option<LaunchboxMeta>> {
    let key = match_key(title);
    if key.is_empty() {
        return Ok(None);
    }
    // Prefer an entry that actually has an overview, then highest rating.
    let row = conn
        .query_row(
            "SELECT name, release_date, overview, developer, publisher, genres
             FROM launchbox_games
             WHERE platform_id = ?1 AND match_key = ?2
             ORDER BY (overview IS NOT NULL) DESC, rating DESC
             LIMIT 1",
            rusqlite::params![platform_id, key],
            |r| {
                Ok(LaunchboxMeta {
                    name: r.get(0)?,
                    release_date: r.get(1)?,
                    overview: r.get(2)?,
                    developer: r.get(3)?,
                    publisher: r.get(4)?,
                    genres: r.get::<_, Option<String>>(5)?.map(|g| {
                        g.split(';')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    }),
                })
            },
        )
        .ok();
    Ok(row)
}

/// Number of cached LaunchBox games (0 = database not downloaded yet).
pub fn cached_count(conn: &rusqlite::Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM launchbox_games", [], |r| r.get(0))?)
}

fn normalize_date(raw: &str) -> Option<String> {
    let t = raw.trim();
    // Full timestamp "1995-10-03T00:00:00+00:00" → date part.
    if t.len() >= 10 && t.as_bytes()[4] == b'-' {
        return Some(t[..10].to_string());
    }
    // Bare year.
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!("{t}-01-01"));
    }
    None
}

#[derive(Default)]
struct GameAccum {
    name: String,
    platform: String,
    release_date: Option<String>,
    release_year: Option<String>,
    overview: Option<String>,
    developer: Option<String>,
    publisher: Option<String>,
    genres: Option<String>,
    rating: Option<f64>,
}

/// Download the Metadata.zip, stream-parse it, and replace the local cache.
/// Reports parsed-game progress via `on_progress`. Honors cancellation.
pub fn download_and_import(
    db: &Db,
    cache_dir: &Path,
    cancel: Arc<AtomicBool>,
    on_progress: impl Fn(u64, u64),
) -> Result<u64> {
    std::fs::create_dir_all(cache_dir)?;
    let zip_path = cache_dir.join("launchbox-metadata.zip");

    // Download to disk (≈100 MB) so we never hold it all in memory.
    on_progress(0, 0);
    let resp = ureq::get(METADATA_URL)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| AppError::Network(format!("failed to download LaunchBox database: {e}")))?;
    {
        let mut out = std::fs::File::create(&zip_path)?;
        let mut reader = resp.into_reader();
        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::Cancelled);
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
        }
        out.flush()?;
    }

    let file = std::fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Invalid(format!("invalid LaunchBox zip: {e}")))?;
    let entry = archive
        .by_name("Metadata.xml")
        .map_err(|e| AppError::Invalid(format!("Metadata.xml missing from zip: {e}")))?;

    let imported = import_from_reader(
        db,
        BufReader::with_capacity(256 * 1024, entry),
        &cancel,
        &on_progress,
    )?;
    let _ = std::fs::remove_file(&zip_path);
    Ok(imported)
}

/// Stream-parse `<Game>` elements from a Metadata.xml reader into the cache.
fn import_from_reader<R: Read>(
    db: &Db,
    reader: R,
    cancel: &AtomicBool,
    on_progress: &impl Fn(u64, u64),
) -> Result<u64> {
    let mut xml = Reader::from_reader(std::io::BufReader::new(reader));
    xml.config_mut().trim_text(true);

    // Clear any previous cache, then bulk-insert in batches inside transactions.
    db.with(|c| {
        c.execute("DELETE FROM launchbox_games", [])?;
        Ok(())
    })?;

    let mut buf = Vec::new();
    let mut field = String::new(); // current element name inside a <Game>
    let mut in_game = false;
    let mut acc = GameAccum::default();
    let mut batch: Vec<GameAccum> = Vec::with_capacity(2000);
    let mut imported: u64 = 0;
    let mut seen: u64 = 0;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
        match xml.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let tag = String::from_utf8_lossy(name.as_ref()).to_string();
                if tag == "Game" {
                    in_game = true;
                    acc = GameAccum::default();
                } else if in_game {
                    field = tag;
                }
            }
            Ok(Event::Text(t)) if in_game && !field.is_empty() => {
                let text = t.unescape().unwrap_or_default().to_string();
                if text.is_empty() {
                    continue;
                }
                match field.as_str() {
                    "Name" => acc.name = text,
                    "Platform" => acc.platform = text,
                    "ReleaseDate" => acc.release_date = normalize_date(&text),
                    "ReleaseYear" => acc.release_year = normalize_date(&text),
                    "Overview" => acc.overview = Some(text),
                    "Developer" => acc.developer = Some(text),
                    "Publisher" => acc.publisher = Some(text),
                    "Genres" => acc.genres = Some(text),
                    "CommunityRating" => acc.rating = text.parse().ok(),
                    _ => {}
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"Game" => {
                in_game = false;
                field.clear();
                seen += 1;
                if platform_for(&acc.platform).is_some() && !acc.name.is_empty() {
                    batch.push(std::mem::take(&mut acc));
                    if batch.len() >= 2000 {
                        imported += flush_batch(db, &mut batch)?;
                        on_progress(imported, seen);
                    }
                }
            }
            Ok(Event::End(_)) if in_game => field.clear(),
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Invalid(format!("LaunchBox XML parse error: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    imported += flush_batch(db, &mut batch)?;
    on_progress(imported, seen);
    tracing::info!(imported, scanned = seen, "LaunchBox database imported");
    Ok(imported)
}

fn flush_batch(db: &Db, batch: &mut Vec<GameAccum>) -> Result<u64> {
    let n = batch.len() as u64;
    db.with(|c| {
        let tx = c.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO launchbox_games
                   (platform_id, match_key, name, release_date, overview, developer, publisher, genres, rating)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for g in batch.iter() {
                let platform = platform_for(&g.platform).unwrap_or("");
                let key = match_key(&g.name);
                if key.is_empty() {
                    continue;
                }
                stmt.execute(rusqlite::params![
                    platform,
                    key,
                    g.name,
                    g.release_date.clone().or_else(|| g.release_year.clone()),
                    g.overview,
                    g.developer,
                    g.publisher,
                    g.genres,
                    g.rating,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })?;
    batch.clear();
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_key_normalizes_titles() {
        assert_eq!(
            match_key("The Legend of Zelda: Ocarina of Time"),
            "legendofzeldaocarinaoftime"
        );
        assert_eq!(
            match_key("Legend of Zelda - Ocarina of Time"),
            "legendofzeldaocarinaoftime"
        );
        assert_eq!(match_key("Sonic the Hedgehog 2"), "sonicthehedgehog2");
    }

    #[test]
    fn date_normalization() {
        assert_eq!(
            normalize_date("1995-10-03T00:00:00+00:00").as_deref(),
            Some("1995-10-03")
        );
        assert_eq!(normalize_date("1994").as_deref(), Some("1994-01-01"));
        assert_eq!(normalize_date(""), None);
    }

    #[test]
    fn platform_mapping_covers_consoles() {
        assert_eq!(platform_for("Sony Playstation"), Some("psx"));
        assert_eq!(platform_for("Nintendo GameCube"), Some("gamecube"));
        assert_eq!(platform_for("Atari 2600"), None);
    }

    /// Validates zip extraction + streaming parse against the real ~495 MB
    /// Metadata.xml. Requires a local `/tmp/lbmeta.zip`; run with `--ignored`.
    #[test]
    #[ignore]
    fn parses_real_metadata_zip() {
        let path = std::path::Path::new("/tmp/lbmeta.zip");
        if !path.exists() {
            eprintln!("skipping: /tmp/lbmeta.zip not present");
            return;
        }
        let file = std::fs::File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let entry = archive.by_name("Metadata.xml").unwrap();
        let db = Db::open_in_memory().unwrap();
        let cancel = AtomicBool::new(false);
        let n = import_from_reader(
            &db,
            BufReader::with_capacity(256 * 1024, entry),
            &cancel,
            &|i, _| {
                if i % 20000 == 0 && i > 0 {
                    eprintln!("{i} imported");
                }
            },
        )
        .unwrap();
        eprintln!("imported {n} games for mapped platforms");
        assert!(
            n > 20000,
            "expected tens of thousands of mapped games, got {n}"
        );
        db.with(|c| {
            let meta = lookup(c, "n64", "Super Mario 64")?.expect("Super Mario 64 on N64");
            assert!(meta.developer.is_some());
            assert!(meta.release_date.is_some());
            eprintln!(
                "Super Mario 64: {:?} / {:?}",
                meta.developer, meta.release_date
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn imports_games_for_mapped_platforms_only() {
        let xml = r#"<LaunchBox>
            <Game><Name>Super Mario 64</Name><Platform>Nintendo 64</Platform>
              <ReleaseDate>1996-06-23T00:00:00+00:00</ReleaseDate>
              <Developer>Nintendo</Developer><Publisher>Nintendo</Publisher>
              <Genres>Platform; Action</Genres><Overview>A 3D platformer.</Overview>
              <CommunityRating>4.5</CommunityRating></Game>
            <Game><Name>Some PC Game</Name><Platform>Windows</Platform></Game>
        </LaunchBox>"#;
        let db = Db::open_in_memory().unwrap();
        let cancel = AtomicBool::new(false);
        let n = import_from_reader(&db, xml.as_bytes(), &cancel, &|_, _| {}).unwrap();
        assert_eq!(n, 1, "only the N64 game maps");

        db.with(|c| {
            let meta = lookup(c, "n64", "Super Mario 64")?.expect("match");
            assert_eq!(meta.release_date.as_deref(), Some("1996-06-23"));
            assert_eq!(meta.developer.as_deref(), Some("Nintendo"));
            assert_eq!(
                meta.genres.as_ref().unwrap(),
                &vec!["Platform".to_string(), "Action".to_string()]
            );
            assert!(meta.overview.is_some());
            // Windows game was skipped.
            assert!(lookup(c, "windows", "Some PC Game")?.is_none());
            Ok(())
        })
        .unwrap();
    }
}
