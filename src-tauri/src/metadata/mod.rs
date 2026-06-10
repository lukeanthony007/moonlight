//! Metadata/artwork provider abstraction.
//!
//! Providers implement [`MetadataProvider`]; the registry instantiates the
//! ones that are configured. The app stays fully functional with none
//! configured — manual editing and local artwork remain available.

pub mod steamgriddb;

use crate::db::repo::settings;
use crate::db::Db;
use crate::error::{AppError, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub title: String,
    pub platform_id: Option<String>,
    pub region: Option<String>,
    pub release_year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMatch {
    pub provider: String,
    pub provider_game_id: String,
    pub title: String,
    pub release_year: Option<i32>,
    /// 0.0–1.0; how confident we are this is the right game.
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCandidate {
    pub provider: String,
    pub kind: String,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Attribution (e.g. the SteamGridDB author) shown in the UI.
    pub author: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Option<Vec<String>>,
}

pub trait MetadataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn search(&self, query: &SearchQuery) -> Result<Vec<ProviderMatch>>;
    fn artwork(&self, provider_game_id: &str, kind: &str) -> Result<Vec<ArtworkCandidate>>;
    fn metadata(&self, provider_game_id: &str) -> Result<FetchedMetadata>;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub requires_api_key: bool,
    pub attribution: String,
}

pub fn provider_statuses(db: &Db) -> Result<Vec<ProviderStatus>> {
    let sgdb_key = db.with(|c| settings::get_string(c, "providers.steamgriddb.apiKey"))?;
    Ok(vec![ProviderStatus {
        id: "steamgriddb".into(),
        name: "SteamGridDB".into(),
        configured: sgdb_key.map(|k| !k.trim().is_empty()).unwrap_or(false),
        requires_api_key: true,
        attribution: "Artwork provided by SteamGridDB (steamgriddb.com)".into(),
    }])
}

/// Instantiate a configured provider by id.
pub fn get_provider(db: &Db, provider_id: &str) -> Result<Box<dyn MetadataProvider>> {
    match provider_id {
        "steamgriddb" => {
            let key = db
                .with(|c| settings::get_string(c, "providers.steamgriddb.apiKey"))?
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| {
                    AppError::Provider(
                        "SteamGridDB API key is not configured. Add one in Settings → Metadata Providers.".into(),
                    )
                })?;
            Ok(Box::new(steamgriddb::SteamGridDb::new(key)))
        }
        other => Err(AppError::Provider(format!("unknown provider: {other}"))),
    }
}

/// Title-similarity confidence score in [0, 1].
pub fn match_confidence(
    query: &SearchQuery,
    candidate_title: &str,
    candidate_year: Option<i32>,
) -> f64 {
    fn tokens(s: &str) -> Vec<String> {
        s.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .map(|t| t.to_string())
            .collect()
    }
    let a = tokens(&query.title);
    let b = tokens(candidate_title);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.iter().filter(|t| b.contains(t)).count() as f64;
    let union = (a.len() + b.len()) as f64 - intersection;
    let mut score = (intersection / union) * 0.9;
    if let (Some(qy), Some(cy)) = (query.release_year, candidate_year) {
        if qy == cy {
            score += 0.1;
        }
    }
    score.min(1.0)
}

/// Simple persistent cache for provider responses (24h TTL).
pub fn cached_or_fetch(
    db: &Db,
    cache_key: &str,
    fetch: impl FnOnce() -> Result<String>,
) -> Result<String> {
    let cached: Option<(String, String)> = db.with(|c| {
        Ok(c.query_row(
            "SELECT payload, fetched_at FROM provider_cache WHERE cache_key = ?1",
            [cache_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map(Some)
        .unwrap_or(None))
    })?;
    if let Some((payload, fetched_at)) = cached {
        if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&fetched_at) {
            if chrono::Utc::now().signed_duration_since(t) < chrono::Duration::hours(24) {
                return Ok(payload);
            }
        }
    }
    let fresh = fetch()?;
    db.with(|c| {
        c.execute(
            "INSERT INTO provider_cache (cache_key, payload, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(cache_key) DO UPDATE SET payload=?2, fetched_at=?3",
            params![cache_key, fresh, crate::db::repo::now()],
        )?;
        Ok(())
    })?;
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(title: &str, year: Option<i32>) -> SearchQuery {
        SearchQuery {
            title: title.into(),
            platform_id: None,
            region: None,
            release_year: year,
        }
    }

    #[test]
    fn exact_title_scores_high() {
        let score = match_confidence(&query("Super Mario 64", None), "Super Mario 64", None);
        assert!(score > 0.85, "score was {score}");
    }

    #[test]
    fn unrelated_title_scores_low() {
        let score = match_confidence(&query("Super Mario 64", None), "Doom Eternal", None);
        assert!(score < 0.2, "score was {score}");
    }

    #[test]
    fn year_match_boosts_score() {
        let without = match_confidence(&query("Doom", Some(1993)), "Doom", None);
        let with = match_confidence(&query("Doom", Some(1993)), "Doom", Some(1993));
        assert!(with > without);
    }
}
