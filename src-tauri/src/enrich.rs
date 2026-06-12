//! Bulk metadata + artwork enrichment.
//!
//! Steam games arrive with the Steam client's cached artwork, but ROM and
//! manual games have no local art source. This job walks every game missing
//! box art, searches the configured metadata provider, and downloads box art,
//! background and logo (plus merges textual metadata, respecting locks).
//!
//! It reuses the scan progress/registry plumbing so the existing UI banner
//! reports progress and results.

use crate::artwork_store;
use crate::db::repo::{artwork, games};
use crate::db::Db;
use crate::domain::ScanReport;
use crate::error::Result;
use crate::metadata::{self, SearchQuery};
use crate::scan::ProgressSink;
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
}

fn missing_artwork_targets(db: &Db, platform_filter: Option<&str>) -> Result<Vec<Target>> {
    db.with(|c| {
        let mut stmt = c.prepare(
            "SELECT g.id, g.title, g.release_date, g.platform_id FROM games g
             WHERE g.platform_id != 'steam'
               AND NOT EXISTS (
                 SELECT 1 FROM artwork a
                 WHERE a.game_id = g.id AND a.kind = 'boxart' AND a.local_path IS NOT NULL
               )
             ORDER BY g.sort_title",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Target {
                id: r.get(0)?,
                title: r.get(1)?,
                release_date: r.get(2)?,
                platform_id: r.get(3)?,
            })
        })?;
        let all = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(all
            .into_iter()
            .filter(|t| platform_filter.is_none_or(|f| f == t.platform_id))
            .collect())
    })
}

pub fn run_enrich(
    db: &Db,
    artwork_dir: &Path,
    provider_id: &str,
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
        let provider = metadata::get_provider(db, provider_id)?;
        let targets = missing_artwork_targets(db, platform_filter)?;
        let total = targets.len() as u32;
        progress.report("matching", 0, total, "Finding artwork");

        for (index, target) in targets.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                report.cancelled = true;
                break;
            }
            progress.report("matching", index as u32 + 1, total, &target.title);

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

            let best = match provider.search(&query) {
                Ok(matches) => matches.into_iter().find(|m| m.confidence >= MIN_CONFIDENCE),
                Err(e) => {
                    report.errors.push(format!("{}: {e}", target.title));
                    if report.errors.len() > 25 {
                        // Likely a bad key or rate-limit wall; stop hammering.
                        report
                            .errors
                            .push("too many provider errors — stopping".into());
                        break;
                    }
                    continue;
                }
            };
            let Some(best) = best else {
                report.skipped += 1;
                continue;
            };

            // Merge textual metadata (locks respected inside the repo call).
            if let Ok(meta) = provider.metadata(&best.provider_game_id) {
                let _ = db.with(|c| {
                    games::merge_provider_metadata(
                        c,
                        &target.id,
                        &games::ProviderMetadata {
                            provider: provider_id.to_string(),
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
                                provider_id,
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
            if got_boxart {
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
