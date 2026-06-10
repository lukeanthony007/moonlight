use super::new_id;
use crate::domain::Artwork;
use crate::error::Result;
use rusqlite::{params, Connection, Row};

pub const KINDS: &[&str] = &[
    "boxart",
    "background",
    "screenshot",
    "logo",
    "icon",
    "banner",
];

fn map(row: &Row) -> rusqlite::Result<Artwork> {
    Ok(Artwork {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        kind: row.get("kind")?,
        local_path: row.get("local_path")?,
        remote_url: row.get("remote_url")?,
        provider: row.get("provider")?,
        user_selected: row.get("user_selected")?,
    })
}

pub fn list_for_game(conn: &Connection, game_id: &str) -> Result<Vec<Artwork>> {
    let mut stmt = conn.prepare("SELECT * FROM artwork WHERE game_id = ?1 ORDER BY kind, rowid")?;
    let rows = stmt.query_map([game_id], map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn has_user_selected(conn: &Connection, game_id: &str, kind: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artwork WHERE game_id = ?1 AND kind = ?2 AND user_selected = 1",
        params![game_id, kind],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

pub struct NewArtwork<'a> {
    pub game_id: &'a str,
    pub kind: &'a str,
    pub local_path: Option<&'a str>,
    pub remote_url: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub user_selected: bool,
}

pub fn insert(conn: &Connection, art: &NewArtwork) -> Result<Artwork> {
    let id = new_id();
    // A user selection replaces any previous selection for that slot.
    if art.user_selected {
        conn.execute(
            "UPDATE artwork SET user_selected = 0 WHERE game_id = ?1 AND kind = ?2",
            params![art.game_id, art.kind],
        )?;
    }
    conn.execute(
        "INSERT INTO artwork (id, game_id, kind, local_path, remote_url, provider, user_selected)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            art.game_id,
            art.kind,
            art.local_path,
            art.remote_url,
            art.provider,
            art.user_selected
        ],
    )?;
    Ok(conn.query_row("SELECT * FROM artwork WHERE id = ?1", [id], map)?)
}

pub fn select(conn: &Connection, artwork_id: &str) -> Result<()> {
    let (game_id, kind): (String, String) = conn.query_row(
        "SELECT game_id, kind FROM artwork WHERE id = ?1",
        [artwork_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute(
        "UPDATE artwork SET user_selected = 0 WHERE game_id = ?1 AND kind = ?2",
        params![game_id, kind],
    )?;
    conn.execute(
        "UPDATE artwork SET user_selected = 1 WHERE id = ?1",
        [artwork_id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, artwork_id: &str) -> Result<Option<String>> {
    let local: Option<String> = conn
        .query_row(
            "SELECT local_path FROM artwork WHERE id = ?1",
            [artwork_id],
            |r| r.get(0),
        )
        .unwrap_or(None);
    conn.execute("DELETE FROM artwork WHERE id = ?1", [artwork_id])?;
    Ok(local)
}
