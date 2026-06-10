use super::json_vec;
use crate::domain::Platform;
use crate::error::Result;
use rusqlite::{params, Connection, Row};

fn map(row: &Row) -> rusqlite::Result<Platform> {
    Ok(Platform {
        id: row.get("id")?,
        name: row.get("name")?,
        short_name: row.get("short_name")?,
        manufacturer: row.get("manufacturer")?,
        default_emulator_id: row.get("default_emulator_id")?,
        extensions: json_vec(row.get("extensions")?),
        sort_order: row.get("sort_order")?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Platform>> {
    let mut stmt = conn.prepare("SELECT * FROM platforms ORDER BY sort_order, name")?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Platform>> {
    let mut stmt = conn.prepare("SELECT * FROM platforms WHERE id = ?1")?;
    let mut rows = stmt.query_map([id], map)?;
    Ok(rows.next().transpose()?)
}

pub fn upsert(conn: &Connection, p: &Platform) -> Result<()> {
    conn.execute(
        "INSERT INTO platforms (id, name, short_name, manufacturer, default_emulator_id, extensions, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET name=?2, short_name=?3, manufacturer=?4,
           default_emulator_id=?5, extensions=?6, sort_order=?7",
        params![
            p.id,
            p.name,
            p.short_name,
            p.manufacturer,
            p.default_emulator_id,
            serde_json::to_string(&p.extensions).unwrap(),
            p.sort_order
        ],
    )?;
    Ok(())
}

/// Insert a platform only if it does not exist (keeps user customizations).
pub fn ensure(conn: &Connection, p: &Platform) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO platforms (id, name, short_name, manufacturer, default_emulator_id, extensions, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            p.id,
            p.name,
            p.short_name,
            p.manufacturer,
            p.default_emulator_id,
            serde_json::to_string(&p.extensions).unwrap(),
            p.sort_order
        ],
    )?;
    Ok(())
}

pub fn set_default_emulator(
    conn: &Connection,
    platform_id: &str,
    emulator_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE platforms SET default_emulator_id = ?1 WHERE id = ?2",
        params![emulator_id, platform_id],
    )?;
    Ok(())
}
