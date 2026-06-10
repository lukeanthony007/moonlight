use crate::error::Result;
use rusqlite::{params, Connection};
use serde_json::Value;

/// All settings as one JSON object keyed by section.
pub fn all(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut obj = serde_json::Map::new();
    for (key, raw) in rows {
        obj.insert(key, serde_json::from_str(&raw).unwrap_or(Value::Null));
    }
    Ok(Value::Object(obj))
}

pub fn get(conn: &Connection, key: &str) -> Result<Option<Value>> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .map(Some)
        .unwrap_or(None);
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn set(conn: &Connection, key: &str, value: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value.to_string()],
    )?;
    Ok(())
}

pub fn get_string(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(get(conn, key)?.and_then(|v| v.as_str().map(|s| s.to_string())))
}

pub fn get_bool(conn: &Connection, key: &str, default: bool) -> Result<bool> {
    Ok(get(conn, key)?.and_then(|v| v.as_bool()).unwrap_or(default))
}
