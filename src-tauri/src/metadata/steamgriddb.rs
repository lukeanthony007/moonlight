//! SteamGridDB provider (https://www.steamgriddb.com).
//!
//! SteamGridDB is artwork-focused: it supplies grids (box art), heroes
//! (backgrounds), logos and icons, plus game names and release dates.

use super::{ArtworkCandidate, FetchedMetadata, MetadataProvider, ProviderMatch, SearchQuery};
use crate::error::{AppError, Result};
use serde_json::Value;
use std::time::Duration;

pub struct SteamGridDb {
    api_key: String,
}

impl SteamGridDb {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    /// GET an endpoint with auth, retrying on 429 with backoff.
    fn get(&self, path: &str) -> Result<Value> {
        let url = format!("https://www.steamgriddb.com/api/v2{path}");
        let mut attempt = 0;
        loop {
            attempt += 1;
            let response = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", self.api_key))
                .timeout(Duration::from_secs(20))
                .call();
            match response {
                Ok(res) => {
                    let body: Value = res.into_json().map_err(|e| {
                        AppError::Provider(format!("invalid SteamGridDB response: {e}"))
                    })?;
                    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
                        return Err(AppError::Provider("SteamGridDB request failed".into()));
                    }
                    return Ok(body);
                }
                Err(ureq::Error::Status(401, _)) => {
                    return Err(AppError::Provider(
                        "SteamGridDB rejected the API key (401)".into(),
                    ));
                }
                Err(ureq::Error::Status(429, _)) if attempt < 4 => {
                    // Rate limited: back off and retry.
                    std::thread::sleep(Duration::from_millis(500 * 2u64.pow(attempt)));
                }
                Err(ureq::Error::Status(code, _)) => {
                    return Err(AppError::Provider(format!("SteamGridDB error {code}")));
                }
                Err(e) => return Err(AppError::Network(format!("SteamGridDB unreachable: {e}"))),
            }
        }
    }

    fn artwork_endpoint(kind: &str) -> Option<&'static str> {
        match kind {
            "boxart" => Some("grids"),
            "background" => Some("heroes"),
            "logo" => Some("logos"),
            "icon" => Some("icons"),
            // SteamGridDB grids in 460x215/920x430 work as banners.
            "banner" => Some("grids"),
            _ => None,
        }
    }
}

impl MetadataProvider for SteamGridDb {
    fn id(&self) -> &'static str {
        "steamgriddb"
    }

    fn name(&self) -> &'static str {
        "SteamGridDB"
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<ProviderMatch>> {
        let encoded = urlencode(&query.title);
        let body = self.get(&format!("/search/autocomplete/{encoded}"))?;
        let empty = Vec::new();
        let items = body
            .get("data")
            .and_then(|d| d.as_array())
            .unwrap_or(&empty);
        let mut matches: Vec<ProviderMatch> = items
            .iter()
            .filter_map(|item| {
                let id = item.get("id")?.as_i64()?;
                let name = item.get("name")?.as_str()?.to_string();
                let year = item
                    .get("release_date")
                    .and_then(|v| v.as_i64())
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|d| {
                        use chrono::Datelike;
                        d.year()
                    });
                let confidence = super::match_confidence(query, &name, year);
                Some(ProviderMatch {
                    provider: "steamgriddb".into(),
                    provider_game_id: id.to_string(),
                    title: name,
                    release_year: year,
                    confidence,
                })
            })
            .collect();
        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(matches)
    }

    fn artwork(&self, provider_game_id: &str, kind: &str) -> Result<Vec<ArtworkCandidate>> {
        let endpoint = Self::artwork_endpoint(kind).ok_or_else(|| {
            AppError::Provider(format!("SteamGridDB has no artwork of kind '{kind}'"))
        })?;
        let dimensions = match kind {
            "boxart" => "?dimensions=600x900,342x482,660x930",
            "banner" => "?dimensions=460x215,920x430",
            _ => "",
        };
        let body = self.get(&format!("/{endpoint}/game/{provider_game_id}{dimensions}"))?;
        let empty = Vec::new();
        let items = body
            .get("data")
            .and_then(|d| d.as_array())
            .unwrap_or(&empty);
        Ok(items
            .iter()
            .filter_map(|item| {
                Some(ArtworkCandidate {
                    provider: "steamgriddb".into(),
                    kind: kind.to_string(),
                    url: item.get("url")?.as_str()?.to_string(),
                    thumbnail_url: item
                        .get("thumb")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    width: item.get("width").and_then(|v| v.as_i64()),
                    height: item.get("height").and_then(|v| v.as_i64()),
                    author: item
                        .get("author")
                        .and_then(|a| a.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect())
    }

    fn metadata(&self, provider_game_id: &str) -> Result<FetchedMetadata> {
        let body = self.get(&format!("/games/id/{provider_game_id}"))?;
        let data = body.get("data").cloned().unwrap_or(Value::Null);
        let release_date = data
            .get("release_date")
            .and_then(|v| v.as_i64())
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map(|d| d.format("%Y-%m-%d").to_string());
        Ok(FetchedMetadata {
            title: data
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            release_date,
            ..Default::default()
        })
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "%20".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_escapes_reserved_characters() {
        assert_eq!(urlencode("Super Mario 64"), "Super%20Mario%2064");
        assert_eq!(urlencode("Pokémon"), "Pok%C3%A9mon");
        assert_eq!(urlencode("a/b&c"), "a%2Fb%26c");
    }
}
