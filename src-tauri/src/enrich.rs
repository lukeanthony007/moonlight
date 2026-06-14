//! Bulk metadata + artwork enrichment.
//!
//! Steam games arrive with the Steam client's cached artwork, but ROM and
//! manual games have no local art source. This job walks every game missing
//! box art and fills it in, preferring two sources per game:
//!
//! 1. **libretro-thumbnails** — a free, no-key public CDN, used for any ROM
//!    platform it covers (matched by No-Intro / Redump filename).
//! 2. The configured **metadata provider** (e.g. SteamGridDB) — fuzzy title
//!    search, used as a fallback and for platforms libretro doesn't cover.
//!
//! At least one source is usually available, so enrichment works with zero
//! configuration. It reuses the scan progress/registry plumbing so the
//! existing UI banner reports progress and results.

use crate::artwork_store;
use crate::db::repo::{artwork, games};
use crate::db::Db;
use crate::domain::ScanReport;
use crate::error::Result;
use crate::libretro_art;
use crate::metadata::{self, MetadataProvider, SearchQuery};
use crate::scan::ProgressSink;
use crate::switch_art;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Minimum match confidence before we trust a provider result enough to apply
/// it automatically. The metadata editor lets users apply weaker matches by
/// hand.
const MIN_CONFIDENCE: f64 = 0.5;

struct Target {
    id: String,
    title: String,
    release_date: Option<String>,
    platform_id: String,
    rom_path: Option<String>,
    /// Steam appid (the steam installation's source id), when applicable.
    steam_appid: Option<String>,
}

fn missing_artwork_targets(db: &Db, platform_filter: Option<&str>) -> Result<Vec<Target>> {
    db.with(|c| {
        // When the LaunchBox database is downloaded, every ROM game without
        // metadata becomes a metadata target too (not just Switch/Steam).
        let has_launchbox = crate::launchbox::cached_count(c).unwrap_or(0) > 0;
        // Target games missing box art, plus games on platforms with a keyless
        // metadata source that haven't been enriched yet — so their names,
        // dates and details get filled in once.
        let mut stmt = c.prepare(
            "SELECT g.id, g.title, g.release_date, g.platform_id,
                    (SELECT i.path FROM installations i
                     WHERE i.game_id = g.id AND i.source_type = 'rom' LIMIT 1) AS rom_path,
                    (SELECT i.source_id FROM installations i
                     WHERE i.game_id = g.id AND i.source_type = 'steam' LIMIT 1) AS steam_appid
             FROM games g
             WHERE ( g.platform_id != 'steam'
                     AND NOT EXISTS (
                       SELECT 1 FROM artwork a
                       WHERE a.game_id = g.id AND a.kind = 'boxart' AND a.local_path IS NOT NULL
                     ) )
                OR ( g.provider_metadata IS NULL
                     AND ( g.platform_id IN ('switch', 'steam') OR ?1 = 1 ) )
             ORDER BY g.sort_title",
        )?;
        let rows = stmt.query_map([has_launchbox], |r| {
            Ok(Target {
                id: r.get(0)?,
                title: r.get(1)?,
                release_date: r.get(2)?,
                platform_id: r.get(3)?,
                rom_path: r.get(4)?,
                steam_appid: r.get(5)?,
            })
        })?;
        let all = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(all
            .into_iter()
            .filter(|t| platform_filter.is_none_or(|f| f == t.platform_id))
            .collect())
    })
}

/// True if enrichment has any source available: a libretro-covered platform
/// among the targets, or a configured metadata provider.
pub fn has_any_source(db: &Db, platform_filter: Option<&str>) -> bool {
    let provider_configured = metadata::provider_statuses(db)
        .map(|s| s.iter().any(|p| p.configured))
        .unwrap_or(false);
    if provider_configured {
        return true;
    }
    missing_artwork_targets(db, platform_filter)
        .map(|t| {
            t.iter().any(|g| {
                libretro_art::system_for(&g.platform_id).is_some()
                    || switch_art::can_match(g.rom_path.as_deref())
                    || g.steam_appid.is_some()
            })
        })
        .unwrap_or(false)
}

/// Try the configured metadata provider for one target. Returns `true` if box
/// art was stored. Errors propagate so the caller can rate-limit.
fn try_provider(
    db: &Db,
    artwork_dir: &Path,
    provider: &dyn MetadataProvider,
    target: &Target,
) -> Result<bool> {
    let year = target
        .release_date
        .as_deref()
        .and_then(|d| d.get(0..4))
        .and_then(|y| y.parse::<i32>().ok());
    let query = SearchQuery {
        title: target.title.clone(),
        platform_id: Some(target.platform_id.clone()),
        region: None,
        release_year: year,
    };
    let best = match provider
        .search(&query)?
        .into_iter()
        .find(|m| m.confidence >= MIN_CONFIDENCE)
    {
        Some(b) => b,
        None => return Ok(false),
    };

    // Merge textual metadata (locks respected inside the repo call).
    if let Ok(meta) = provider.metadata(&best.provider_game_id) {
        let _ = db.with(|c| {
            games::merge_provider_metadata(
                c,
                &target.id,
                &games::ProviderMetadata {
                    provider: provider.id().to_string(),
                    title: meta.title,
                    description: meta.description,
                    release_date: meta.release_date,
                    developer: meta.developer,
                    publisher: meta.publisher,
                    genres: meta.genres,
                },
            )
        });
    }

    let mut got_boxart = false;
    for kind in ["boxart", "background", "logo"] {
        if db
            .with(|c| artwork::has_user_selected(c, &target.id, kind))
            .unwrap_or(false)
        {
            continue; // never displace a user-selected image
        }
        match provider.artwork(&best.provider_game_id, kind) {
            Ok(candidates) => {
                if let Some(candidate) = candidates.first() {
                    match artwork_store::download_url(
                        db,
                        artwork_dir,
                        &target.id,
                        kind,
                        &candidate.url,
                        provider.id(),
                        false,
                    ) {
                        Ok(_) if kind == "boxart" => got_boxart = true,
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!(kind, error = %e, "enrich artwork download failed")
                        }
                    }
                }
            }
            Err(e) => tracing::warn!(kind, error = %e, "enrich artwork lookup failed"),
        }
    }
    Ok(got_boxart)
}

/// `provider_id` is the optional configured metadata provider (e.g.
/// SteamGridDB). libretro thumbnails are always tried first for covered
/// platforms, with no key required.
pub fn run_enrich(
    db: &Db,
    artwork_dir: &Path,
    provider_id: Option<&str>,
    platform_filter: Option<&str>,
    scan_id: String,
    cancel: Arc<AtomicBool>,
    app: Option<&tauri::AppHandle>,
) -> ScanReport {
    let started = std::time::Instant::now();
    let progress = ProgressSink {
        app,
        scan_id: scan_id.clone(),
        source: "artwork".into(),
    };
    let mut report = ScanReport {
        scan_id: scan_id.clone(),
        source: "artwork".into(),
        ..Default::default()
    };

    let result: Result<()> = (|| {
        let provider = match provider_id {
            Some(id) => metadata::get_provider(db, id).ok(),
            None => None,
        };
        let targets = missing_artwork_targets(db, platform_filter)?;
        let total = targets.len() as u32;
        progress.report("matching", 0, total, "Finding artwork");

        let mut provider_errors = 0u32;
        for (index, target) in targets.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                report.cancelled = true;
                break;
            }
            progress.report("matching", index as u32 + 1, total, &target.title);

            // 0. Switch: pull the official eShop name / publisher / description
            //    so e.g. "TOTK" becomes its real title. Respects locked fields.
            if target.platform_id == "switch" {
                if let Some(tid) = target
                    .rom_path
                    .as_deref()
                    .and_then(switch_art::extract_title_id_from_path)
                {
                    if let Some(meta) = switch_art::fetch_metadata(tid) {
                        let _ = db.with(|c| {
                            games::merge_provider_metadata(
                                c,
                                &target.id,
                                &games::ProviderMetadata {
                                    provider: "nintendo".into(),
                                    title: meta.name,
                                    description: meta.description,
                                    release_date: None,
                                    developer: None,
                                    publisher: meta.publisher,
                                    genres: None,
                                },
                            )
                        });
                    }
                }
            }

            // 0b. Steam: pull release date / developer / publisher / genres /
            //     description from the keyless store API (enables date sorting).
            if let Some(appid) = &target.steam_appid {
                if let Some(meta) = crate::steam_store::fetch_metadata(appid) {
                    let _ = db.with(|c| {
                        games::merge_provider_metadata(
                            c,
                            &target.id,
                            &games::ProviderMetadata {
                                provider: "steam".into(),
                                title: meta.name,
                                description: meta.description,
                                release_date: meta.release_date,
                                developer: meta.developer,
                                publisher: meta.publisher,
                                genres: meta.genres,
                            },
                        )
                    });
                }
            }

            // 0c. Console ROMs (incl. Switch): match against the local LaunchBox
            //     Games DB for description / developer / publisher / genres /
            //     release date. Merges respect locks, so the Switch eShop name
            //     and Steam store fields fetched above are preserved.
            if target.platform_id != "steam" {
                if let Ok(Some(meta)) =
                    db.with(|c| crate::launchbox::lookup(c, &target.platform_id, &target.title))
                {
                    let _ = db.with(|c| {
                        games::merge_provider_metadata(
                            c,
                            &target.id,
                            &games::ProviderMetadata {
                                provider: "launchbox".into(),
                                title: None, // keep the title we derived from the file
                                description: meta.overview,
                                release_date: meta.release_date,
                                developer: meta.developer,
                                publisher: meta.publisher,
                                genres: meta.genres,
                            },
                        )
                    });
                }
            }

            // Skip artwork lookups when box art is already present (e.g. a
            // Switch game re-targeted only to fetch its official metadata).
            let mut got = db
                .with(|c| {
                    Ok(c.query_row(
                        "SELECT EXISTS(SELECT 1 FROM artwork WHERE game_id = ?1 AND kind = 'boxart' AND local_path IS NOT NULL)",
                        [&target.id],
                        |r| r.get::<_, bool>(0),
                    )?)
                })
                .unwrap_or(false);

            // 1. libretro thumbnails (free, no key) for covered platforms.
            if !got {
                if let Some(system) = libretro_art::system_for(&target.platform_id) {
                    match libretro_art::fetch_for_game(
                        db,
                        artwork_dir,
                        &target.id,
                        system,
                        &target.title,
                        target.rom_path.as_deref(),
                    ) {
                        Ok(true) => got = true,
                        Ok(false) => {}
                        Err(e) => {
                            tracing::warn!(game = %target.title, error = %e, "libretro art failed")
                        }
                    }
                }
            }

            // 2. Nintendo Switch art via title ID (free, no key).
            if !got && target.platform_id == "switch" {
                match switch_art::fetch_for_game(
                    db,
                    artwork_dir,
                    &target.id,
                    target.rom_path.as_deref(),
                ) {
                    Ok(true) => got = true,
                    Ok(false) => {}
                    Err(e) => tracing::warn!(game = %target.title, error = %e, "switch art failed"),
                }
            }

            // 3. Fall back to the configured provider (fuzzy title search).
            if !got {
                if let Some(provider) = provider.as_deref() {
                    match try_provider(db, artwork_dir, provider, target) {
                        Ok(true) => got = true,
                        Ok(false) => {}
                        Err(e) => {
                            provider_errors += 1;
                            report.errors.push(format!("{}: {e}", target.title));
                            if provider_errors > 25 {
                                report
                                    .errors
                                    .push("too many provider errors — stopping".into());
                                break;
                            }
                        }
                    }
                }
            }

            if got {
                report.updated += 1;
            } else {
                report.skipped += 1;
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        report.errors.push(e.to_string());
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    tracing::info!(
        scan = %scan_id,
        matched = report.updated,
        skipped = report.skipped,
        errors = report.errors.len(),
        "artwork enrichment finished"
    );
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{games, new_id};
    use rusqlite::params;

    fn seed(db: &Db, title: &str, rom_path: &str) {
        db.with(|c| {
            let game = games::insert(
                c,
                &games::NewGame {
                    title,
                    platform_id: "gamecube",
                    release_date: None,
                    region: None,
                },
            )?;
            c.execute(
                "INSERT INTO installations (id, game_id, source_type, source_id, path, installed)
                 VALUES (?1, ?2, 'rom', ?3, ?3, 1)",
                params![new_id(), game.id, rom_path],
            )?;
            Ok(())
        })
        .unwrap();
    }

    /// End-to-end against the live libretro CDN: seeds real GameCube ROM names
    /// and verifies box art actually downloads. Network-dependent, so ignored
    /// by default — run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn libretro_enrichment_downloads_real_boxart() {
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name) VALUES ('gamecube','GameCube','GCN')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        // Region stripped from the title; the raw ROM filename keeps it.
        seed(&db, "F-Zero GX", "/roms/F-Zero GX (USA).ciso");
        seed(
            &db,
            "Animal Crossing",
            "/roms/Animal Crossing (Europe) (En,Fr,De,Es,It).ciso",
        );
        seed(&db, "Amazing Island", "/roms/Amazing Island (USA).nkit.iso");

        let dir = tempfile::tempdir().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let report = run_enrich(
            &db,
            dir.path(),
            None,
            Some("gamecube"),
            "test".into(),
            cancel,
            None,
        );

        assert!(
            report.updated >= 3,
            "expected all 3 to match, report = {report:?}"
        );
        let art_count: i64 = db
            .with(|c| {
                Ok(c.query_row(
                    "SELECT COUNT(*) FROM artwork WHERE kind='boxart' AND provider='libretro' AND local_path IS NOT NULL",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(art_count, 3);

        // The downloaded files exist on disk and are non-trivial PNGs.
        let files: Vec<String> = db
            .with(|c| {
                let mut stmt = c.prepare("SELECT local_path FROM artwork WHERE kind='boxart'")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            })
            .unwrap();
        for path in files {
            let size = std::fs::metadata(&path).unwrap().len();
            assert!(size > 1000, "{path} is suspiciously small ({size} bytes)");
        }
    }
}
