use super::{new_id, now};
use crate::domain::PlaySession;
use crate::error::Result;
use rusqlite::{params, Connection, Row};

fn map(row: &Row) -> rusqlite::Result<PlaySession> {
    Ok(PlaySession {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        duration_seconds: row.get("duration_seconds")?,
        exit_status: row.get("exit_status")?,
    })
}

pub fn start(conn: &Connection, game_id: &str) -> Result<PlaySession> {
    let id = new_id();
    conn.execute(
        "INSERT INTO play_sessions (id, game_id, started_at, exit_status) VALUES (?1, ?2, ?3, 'running')",
        params![id, game_id, now()],
    )?;
    Ok(conn.query_row("SELECT * FROM play_sessions WHERE id = ?1", [id], map)?)
}

pub fn end(
    conn: &Connection,
    session_id: &str,
    duration_seconds: i64,
    exit_status: &str,
) -> Result<PlaySession> {
    conn.execute(
        "UPDATE play_sessions SET ended_at = ?1, duration_seconds = ?2, exit_status = ?3 WHERE id = ?4",
        params![now(), duration_seconds, exit_status, session_id],
    )?;
    Ok(conn.query_row(
        "SELECT * FROM play_sessions WHERE id = ?1",
        [session_id],
        map,
    )?)
}

/// Close any sessions left open by a crash so playtime stats stay sane.
pub fn close_dangling(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE play_sessions SET ended_at = started_at, duration_seconds = 0, exit_status = 'interrupted'
         WHERE exit_status = 'running'",
        [],
    )?;
    Ok(n)
}

pub fn for_game(conn: &Connection, game_id: &str, limit: u32) -> Result<Vec<PlaySession>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM play_sessions WHERE game_id = ?1 ORDER BY started_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![game_id, limit], map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn recent(conn: &Connection, limit: u32) -> Result<Vec<PlaySession>> {
    let mut stmt = conn.prepare("SELECT * FROM play_sessions ORDER BY started_at DESC LIMIT ?1")?;
    let rows = stmt.query_map([limit], map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games::{insert, NewGame};
    use crate::db::Db;

    #[test]
    fn session_lifecycle_records_duration_and_status() {
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name) VALUES ('nes','NES','NES')",
                [],
            )?;
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            let session = start(c, &game.id)?;
            assert_eq!(session.exit_status.as_deref(), Some("running"));
            let ended = end(c, &session.id, 125, "ok")?;
            assert_eq!(ended.duration_seconds, Some(125));
            assert_eq!(ended.exit_status.as_deref(), Some("ok"));
            assert!(ended.ended_at.is_some());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn dangling_sessions_are_closed() {
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO platforms (id, name, short_name) VALUES ('nes','NES','NES')",
                [],
            )?;
            let game = insert(
                c,
                &NewGame {
                    title: "Metroid",
                    platform_id: "nes",
                    release_date: None,
                    region: None,
                },
            )?;
            start(c, &game.id)?;
            start(c, &game.id)?;
            assert_eq!(close_dangling(c)?, 2);
            assert_eq!(close_dangling(c)?, 0);
            Ok(())
        })
        .unwrap();
    }
}
