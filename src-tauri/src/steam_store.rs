//! Keyless Steam store metadata (release date, developers, publishers, genres,
//! description) via the public `appdetails` endpoint. No credentials required.
//!
//! `https://store.steampowered.com/api/appdetails?appids={appid}` →
//! `{"<appid>":{"success":true,"data":{...}}}`.

use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct SteamStoreMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    /// ISO-8601 (`YYYY-MM-DD`); year-only dates become `YYYY-01-01`.
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Option<Vec<String>>,
}

/// Parse a Steam store date string into ISO-8601. Steam returns localized
/// forms like `Apr 18, 2011`, `18 Apr, 2011`, `Oct 2007`, or just `2007`.
pub fn parse_store_date(raw: &str) -> Option<String> {
    use chrono::NaiveDate;
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Comma-delimited full dates only — a no-comma "%b %d %Y" form ambiguously
    // splits "Oct 2007" into day+year, so it is intentionally excluded.
    for fmt in ["%b %d, %Y", "%d %b, %Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    // "Oct 2007" → first of month.
    if let Ok(d) = NaiveDate::parse_from_str(&format!("{s} 1"), "%b %Y %d") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    // Bare year, possibly inside other text ("Q4 2024", "2007"): take the last
    // standalone 4-digit run (the year follows any quarter/month qualifier).
    let year = s
        .split(|c: char| !c.is_ascii_digit())
        .rfind(|tok| tok.len() == 4);
    year.map(|y| format!("{y}-01-01"))
}

fn first_string(value: Option<&Value>) -> Option<String> {
    value?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Fetch store metadata for one Steam app. Returns `None` on network error,
/// rate-limit, or an app with no store page.
pub fn fetch_metadata(appid: &str) -> Option<SteamStoreMeta> {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={appid}&l=english");
    let body: Value = ureq::get(&url)
        .timeout(Duration::from_secs(20))
        .call()
        .ok()?
        .into_json()
        .ok()?;

    let entry = body.get(appid)?;
    if entry.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }
    let data = entry.get("data")?;

    let genres = data.get("genres").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|g| g.get("description").and_then(|d| d.as_str()))
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    });

    let meta = SteamStoreMeta {
        name: data
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        description: data
            .get("short_description")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        release_date: data
            .get("release_date")
            .and_then(|rd| rd.get("date"))
            .and_then(|v| v.as_str())
            .and_then(parse_store_date),
        developer: first_string(data.get("developers")),
        publisher: first_string(data.get("publishers")),
        genres: genres.filter(|g| !g.is_empty()),
    };
    if meta.release_date.is_none() && meta.description.is_none() && meta.developer.is_none() {
        return None;
    }
    Some(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_steam_date_forms() {
        assert_eq!(
            parse_store_date("Apr 18, 2011").as_deref(),
            Some("2011-04-18")
        );
        assert_eq!(
            parse_store_date("18 Apr, 2011").as_deref(),
            Some("2011-04-18")
        );
        assert_eq!(parse_store_date("Oct 2007").as_deref(), Some("2007-10-01"));
        assert_eq!(parse_store_date("2007").as_deref(), Some("2007-01-01"));
        assert_eq!(parse_store_date("Q4 2024").as_deref(), Some("2024-01-01"));
        assert_eq!(parse_store_date("Coming soon"), None);
        assert_eq!(parse_store_date(""), None);
    }

    /// Live keyless Steam store fetch. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn fetches_real_store_metadata() {
        let meta = fetch_metadata("620").expect("Portal 2 store data");
        assert_eq!(meta.release_date.as_deref(), Some("2011-04-18"));
        assert_eq!(meta.developer.as_deref(), Some("Valve"));
        assert!(meta.genres.unwrap().iter().any(|g| g == "Action"));
    }
}
