use super::{json_vec, new_id, now};
use crate::domain::{Game, GamePatch, Installation, LibraryEntry};
use crate::error::{AppError, Result};
use rusqlite::{params, Connection, Row};
use std::collections::HashMap;

pub fn map_game(row: &Row) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get("id")?,
        title: row.get("title")?,
        sort_title: row.get("sort_title")?,
        description: row.get("description")?,
        release_date: row.get("release_date")?,
        developer: row.get("developer")?,
        publisher: row.get("publisher")?,
        genres: json_vec(row.get("genres")?),
        region: row.get("region")?,
        platform_id: row.get("platform_id")?,
        series: row.get("series")?,
        favorite: row.get("favorite")?,
        hidden: row.get("hidden")?,
        date_added: row.get("date_added")?,
        last_played: row.get("last_played")?,
        playtime_seconds: row.get("playtime_seconds")?,
        user_rating: row.get("user_rating")?,
        locked_fields: json_vec(row.get("locked_fields")?),
        provider_metadata: row
            .get::<_, Option<String>>("provider_metadata")?
            .and_then(|s| serde_json::from_str(&s).ok()),
    })
}

pub fn map_installation(row: &Row) -> rusqlite::Result<Installation> {
    Ok(Installation {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        source_type: row.get("source_type")?,
        source_id: row.get("source_id")?,
        path: row.get("path")?,
        emulator_id: row.get("emulator_id")?,
        launch_args: row.get("launch_args")?,
        working_directory: row.get("working_directory")?,
        installed: row.get("installed")?,
        file_size: row.get("file_size")?,
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<Game> {
    conn.query_row("SELECT * FROM games WHERE id = ?1", [id], map_game)
        .map_err(|_| AppError::NotFound(format!("game {id} not found")))
}

pub fn installations_for(conn: &Connection, game_id: &str) -> Result<Vec<Installation>> {
    let mut stmt = conn.prepare("SELECT * FROM installations WHERE game_id = ?1")?;
    let rows = stmt.query_map([game_id], map_installation)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// All games with installations and effective artwork, in one query pass.
pub fn list_library(conn: &Connection) -> Result<Vec<LibraryEntry>> {
    let mut stmt = conn.prepare("SELECT * FROM games")?;
    let games = stmt
        .query_map([], map_game)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut installs_by_game: HashMap<String, Vec<Installation>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT * FROM installations")?;
    for inst in stmt.query_map([], map_installation)? {
        let inst = inst?;
        installs_by_game
            .entry(inst.game_id.clone())
            .or_default()
            .push(inst);
    }

    // Effective artwork per (game, kind): user-selected wins, else newest row.
    let mut art_by_game: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT game_id, kind, local_path FROM artwork
         WHERE local_path IS NOT NULL
         ORDER BY user_selected ASC, rowid ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (game_id, kind, path) = row?;
        // Later rows overwrite earlier ones; ordering puts user_selected last.
        art_by_game.entry(game_id).or_default().insert(kind, path);
    }

    Ok(games
        .into_iter()
        .map(|game| {
            let installations = installs_by_game.remove(&game.id).unwrap_or_default();
            let artwork = art_by_game.remove(&game.id).unwrap_or_default();
            LibraryEntry {
                game,
                installations,
                artwork,
            }
        })
        .collect())
}

pub fn entry(conn: &Connection, id: &str) -> Result<LibraryEntry> {
    let game = get(conn, id)?;
    let installations = installations_for(conn, id)?;
    let mut artwork = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT kind, local_path FROM artwork
         WHERE game_id = ?1 AND local_path IS NOT NULL
         ORDER BY user_selected ASC, rowid ASC",
    )?;
    for row in stmt.query_map([id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (kind, path) = row?;
        artwork.insert(kind, path);
    }
    Ok(LibraryEntry {
        game,
        installations,
        artwork,
    })
}

pub struct NewGame<'a> {
    pub title: &'a str,
    pub platform_id: &'a str,
    pub release_date: Option<&'a str>,
    pub region: Option<&'a str>,
}

pub fn sort_title_for(title: &str) -> String {
    let lower = title.to_lowercase();
    for prefix in ["the ", "a ", "an "] {
        if let Some(stripped) = lower.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }
    lower
}

pub fn insert(conn: &Connection, new: &NewGame) -> Result<Game> {
    let id = new_id();
    conn.execute(
        "INSERT INTO games (id, title, sort_title, release_date, region, platform_id, date_added)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            new.title,
            sort_title_for(new.title),
            new.release_date,
            new.region,
            new.platform_id,
            now()
        ],
    )?;
    get(conn, &id)
}

/// Apply a user-driven metadata patch. User edits always win, so this ignores
/// field locks (locks only guard against *automatic* provider updates).
pub fn apply_patch(conn: &Connection, id: &str, patch: &GamePatch) -> Result<Game> {
    let current = get(conn, id)?;
    let title = patch.title.clone().unwrap_or(current.title);
    let sort_title = match &patch.sort_title {
        Some(s) if !s.trim().is_empty() => s.clone(),
        Some(_) => sort_title_for(&title),
        None if patch.title.is_some() => sort_title_for(&title),
        None => current.sort_title,
    };
    let description = patch.description.clone().unwrap_or(current.description);
    let release_date = patch.release_date.clone().unwrap_or(current.release_date);
    let developer = patch.developer.clone().unwrap_or(current.developer);
    let publisher = patch.publisher.clone().unwrap_or(current.publisher);
    let genres = patch.genres.clone().unwrap_or(current.genres);
    let region = patch.region.clone().unwrap_or(current.region);
    let series = patch.series.clone().unwrap_or(current.series);
    let user_rating = patch.user_rating.unwrap_or(current.user_rating);
    let locked_fields = patch.locked_fields.clone().unwrap_or(current.locked_fields);

    conn.execute(
        "UPDATE games SET title=?1, sort_title=?2, description=?3, release_date=?4, developer=?5,
         publisher=?6, genres=?7, region=?8, series=?9, user_rating=?10, locked_fields=?11
         WHERE id=?12",
        params![
            title,
            sort_title,
            description,
            release_date,
            developer,
            publisher,
            serde_json::to_string(&genres).unwrap(),
            region,
            series,
            user_rating,
            serde_json::to_string(&locked_fields).unwrap(),
            id
        ],
    )?;
    get(conn, id)
}

/// Provider metadata candidate, applied during automatic fetches.
pub struct ProviderMetadata {
    pub provider: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Option<Vec<String>>,
}

/// Merge provider metadata into a game, respecting locked fields and never
/// blanking existing values with empty provider data. Returns the updated game.
pub fn merge_provider_metadata(
    conn: &Connection,
    id: &str,
    meta: &ProviderMetadata,
) -> Result<Game> {
    let current = get(conn, id)?;
    let locked = |field: &str| current.locked_fields.iter().any(|f| f == field);

    let pick =
        |field: &str, incoming: &Option<String>, existing: &Option<String>| -> Option<String> {
            if locked(field) {
                return existing.clone();
            }
            match incoming {
                Some(v) if !v.trim().is_empty() => Some(v.clone()),
                _ => existing.clone(),
            }
        };

    let title = if locked("title") {
        current.title.clone()
    } else {
        meta.title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(current.title.clone())
    };
    let description = pick("description", &meta.description, &current.description);
    let release_date = pick("releaseDate", &meta.release_date, &current.release_date);
    let developer = pick("developer", &meta.developer, &current.developer);
    let publisher = pick("publisher", &meta.publisher, &current.publisher);
    let genres = if locked("genres") {
        current.genres.clone()
    } else {
        match &meta.genres {
            Some(g) if !g.is_empty() => g.clone(),
            _ => current.genres.clone(),
        }
    };

    let snapshot = serde_json::json!({
        "provider": meta.provider,
        "title": meta.title,
        "description": meta.description,
        "releaseDate": meta.release_date,
        "developer": meta.developer,
        "publisher": meta.publisher,
        "genres": meta.genres,
        "fetchedAt": now(),
    });

    conn.execute(
        "UPDATE games SET title=?1, sort_title=?2, description=?3, release_date=?4, developer=?5,
         publisher=?6, genres=?7, provider_metadata=?8 WHERE id=?9",
        params![
            title,
            sort_title_for(&title),
            description,
            release_date,
            developer,
            publisher,
            serde_json::to_string(&genres).unwrap(),
            snapshot.to_string(),
            id
        ],
    )?;
    get(conn, id)
}

/// Restore the last provider snapshot over the current metadata (explicit user
/// action, so locks are not consulted).
pub fn restore_provider_metadata(conn: &Connection, id: &str) -> Result<Game> {
    let current = get(conn, id)?;
    let snapshot = current
        .provider_metadata
        .ok_or_else(|| AppError::Invalid("no provider metadata stored for this game".into()))?;
    let s = |k: &str| {
        snapshot
            .get(k)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
    };
    let title = s("title").unwrap_or(current.title);
    let genres: Vec<String> = snapshot
        .get("genres")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    conn.execute(
        "UPDATE games SET title=?1, sort_title=?2, description=?3, release_date=?4, developer=?5,
         publisher=?6, genres=?7 WHERE id=?8",
        params![
            title,
            sort_title_for(&title),
            s("description"),
            s("releaseDate"),
            s("developer"),
            s("publisher"),
            serde_json::to_string(&genres).unwrap(),
            id
        ],
    )?;
    get(conn, id)
}

pub fn set_favorite(conn: &Connection, id: &str, favorite: bool) -> Result<()> {
    conn.execute(
        "UPDATE games SET favorite = ?1 WHERE id = ?2",
        params![favorite, id],
    )?;
    Ok(())
}

pub fn set_hidden(conn: &Connection, id: &str, hidden: bool) -> Result<()> {
    conn.execute(
        "UPDATE games SET hidden = ?1 WHERE id = ?2",
        params![hidden, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
    Ok(())
}

pub fn record_play(conn: &Connection, id: &str, played_at: &str, seconds: i64) -> Result<()> {
    conn.execute(
        "UPDATE games SET last_played = ?1, playtime_seconds = playtime_seconds + ?2 WHERE id = ?3",
        params![played_at, seconds.max(0), id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn setup() -> Db {
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name) VALUES ('nes','NES','NES')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    #[test]
    fn sort_title_strips_articles() {
        assert_eq!(sort_title_for("The Legend of Zelda"), "legend of zelda");
        assert_eq!(sort_title_for("A Hat in Time"), "hat in time");
        assert_eq!(sort_title_for("Metroid"), "metroid");
    }

    #[test]
    fn patch_updates_only_provided_fields() {
        let db = setup();
        db.with(|c| {
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            let patch = GamePatch {
                description: Some(Some("Classic.".into())),
                user_rating: Some(Some(5)),
                ..Default::default()
            };
            let updated = apply_patch(c, &game.id, &patch)?;
            assert_eq!(updated.title, "Metroid");
            assert_eq!(updated.description.as_deref(), Some("Classic."));
            assert_eq!(updated.user_rating, Some(5));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn provider_merge_respects_locked_fields() {
        let db = setup();
        db.with(|c| {
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            apply_patch(
                c,
                &game.id,
                &GamePatch {
                    description: Some(Some("My own words".into())),
                    locked_fields: Some(vec!["description".into()]),
                    ..Default::default()
                },
            )?;
            let merged = merge_provider_metadata(
                c,
                &game.id,
                &ProviderMetadata {
                    provider: "test".into(),
                    title: Some("Metroid (1986)".into()),
                    description: Some("Provider blurb".into()),
                    release_date: Some("1986-08-06".into()),
                    developer: Some("Nintendo R&D1".into()),
                    publisher: None,
                    genres: Some(vec!["Action".into()]),
                },
            )?;
            // Locked field untouched, unlocked fields updated.
            assert_eq!(merged.description.as_deref(), Some("My own words"));
            assert_eq!(merged.title, "Metroid (1986)");
            assert_eq!(merged.release_date.as_deref(), Some("1986-08-06"));
            assert_eq!(merged.genres, vec!["Action"]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn provider_merge_never_blanks_existing_values() {
        let db = setup();
        db.with(|c| {
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            apply_patch(
                c,
                &game.id,
                &GamePatch {
                    developer: Some(Some("Nintendo".into())),
                    ..Default::default()
                },
            )?;
            let merged = merge_provider_metadata(
                c,
                &game.id,
                &ProviderMetadata {
                    provider: "test".into(),
                    title: None,
                    description: None,
                    release_date: None,
                    developer: Some("  ".into()),
                    publisher: None,
                    genres: Some(vec![]),
                },
            )?;
            assert_eq!(merged.developer.as_deref(), Some("Nintendo"));
            assert_eq!(merged.title, "Metroid");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn restore_provider_metadata_reapplies_snapshot() {
        let db = setup();
        db.with(|c| {
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            merge_provider_metadata(
                c,
                &game.id,
                &ProviderMetadata {
                    provider: "test".into(),
                    title: Some("Metroid".into()),
                    description: Some("Provider blurb".into()),
                    release_date: None,
                    developer: None,
                    publisher: None,
                    genres: None,
                },
            )?;
            apply_patch(
                c,
                &game.id,
                &GamePatch {
                    description: Some(Some("edited".into())),
                    ..Default::default()
                },
            )?;
            let restored = restore_provider_metadata(c, &game.id)?;
            assert_eq!(restored.description.as_deref(), Some("Provider blurb"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn record_play_accumulates() {
        let db = setup();
        db.with(|c| {
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            record_play(c, &game.id, "2026-01-01T00:00:00Z", 600)?;
            record_play(c, &game.id, "2026-01-02T00:00:00Z", 300)?;
            let g = get(c, &game.id)?;
            assert_eq!(g.playtime_seconds, 900);
            assert_eq!(g.last_played.as_deref(), Some("2026-01-02T00:00:00Z"));
            Ok(())
        })
        .unwrap();
    }
}
