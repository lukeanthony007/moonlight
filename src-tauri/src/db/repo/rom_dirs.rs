use super::new_id;
use crate::domain::RomDirectory;
use crate::error::{AppError, Result};
use rusqlite::{params, Connection, Row};

fn map(row: &Row) -> rusqlite::Result<RomDirectory> {
    Ok(RomDirectory {
        id: row.get("id")?,
        path: row.get("path")?,
        platform_id: row.get("platform_id")?,
        emulator_id: row.get("emulator_id")?,
        enabled: row.get("enabled")?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<RomDirectory>> {
    let mut stmt = conn.prepare("SELECT * FROM rom_directories ORDER BY path")?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<RomDirectory> {
    conn.query_row("SELECT * FROM rom_directories WHERE id = ?1", [id], map)
        .map_err(|_| AppError::NotFound(format!("ROM directory {id} not found")))
}

pub fn save(conn: &Connection, mut dir: RomDirectory) -> Result<RomDirectory> {
    if dir.path.trim().is_empty() {
        return Err(AppError::Invalid("directory path is required".into()));
    }
    if dir.id.is_empty() {
        dir.id = new_id();
    }
    conn.execute(
        "INSERT INTO rom_directories (id, path, platform_id, emulator_id, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET path=?2, platform_id=?3, emulator_id=?4, enabled=?5",
        params![
            dir.id,
            dir.path,
            dir.platform_id,
            dir.emulator_id,
            dir.enabled
        ],
    )?;
    Ok(dir)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM rom_directories WHERE id = ?1", [id])?;
    Ok(())
}
