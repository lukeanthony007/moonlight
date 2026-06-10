use super::{new_id, now};
use crate::domain::Collection;
use crate::error::Result;
use rusqlite::{params, Connection};

pub fn list(conn: &Connection) -> Result<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, sort_order FROM collections ORDER BY sort_order, name",
    )?;
    let mut collections = stmt
        .query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                sort_order: r.get(3)?,
                game_ids: vec![],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut stmt = conn.prepare("SELECT collection_id, game_id FROM collection_games")?;
    let pairs = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (collection_id, game_id) in pairs {
        if let Some(c) = collections.iter_mut().find(|c| c.id == collection_id) {
            c.game_ids.push(game_id);
        }
    }
    Ok(collections)
}

pub fn create(conn: &Connection, name: &str) -> Result<Collection> {
    let id = new_id();
    let created_at = now();
    conn.execute(
        "INSERT INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, created_at],
    )?;
    Ok(Collection {
        id,
        name: name.to_string(),
        created_at,
        sort_order: 0,
        game_ids: vec![],
    })
}

pub fn rename(conn: &Connection, id: &str, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE collections SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

pub fn add_game(conn: &Connection, collection_id: &str, game_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
        params![collection_id, game_id],
    )?;
    Ok(())
}

pub fn remove_game(conn: &Connection, collection_id: &str, game_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
        params![collection_id, game_id],
    )?;
    Ok(())
}
