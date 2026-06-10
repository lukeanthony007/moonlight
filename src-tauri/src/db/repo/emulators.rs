use super::{json_map, json_vec, new_id};
use crate::domain::Emulator;
use crate::error::{AppError, Result};
use rusqlite::{params, Connection, Row};

fn map(row: &Row) -> rusqlite::Result<Emulator> {
    Ok(Emulator {
        id: row.get("id")?,
        name: row.get("name")?,
        executable_path: row.get("executable_path")?,
        emulator_type: row.get("emulator_type")?,
        platforms: json_vec(row.get("platforms")?),
        command_template: row.get("command_template")?,
        core_name: row.get("core_name")?,
        working_directory: row.get("working_directory")?,
        environment: json_map(row.get("environment")?),
        enabled: row.get("enabled")?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Emulator>> {
    let mut stmt = conn.prepare("SELECT * FROM emulators ORDER BY name")?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Emulator> {
    conn.query_row("SELECT * FROM emulators WHERE id = ?1", [id], map)
        .map_err(|_| AppError::NotFound(format!("emulator {id} not found")))
}

/// Validate an emulator configuration before saving or launching.
pub fn validate(emulator: &Emulator) -> Result<()> {
    if emulator.name.trim().is_empty() {
        return Err(AppError::Invalid("emulator name is required".into()));
    }
    if emulator.executable_path.trim().is_empty() {
        return Err(AppError::Invalid("executable path is required".into()));
    }
    let template = emulator.command_template.trim();
    if template.is_empty() {
        return Err(AppError::Invalid("command template is required".into()));
    }
    if !template.contains("{executable}") {
        return Err(AppError::Invalid(
            "command template must contain {executable}".into(),
        ));
    }
    if emulator.emulator_type == "retroarch" && !template.contains("{corePath}") {
        return Err(AppError::Invalid(
            "RetroArch command template must contain {corePath}".into(),
        ));
    }
    Ok(())
}

pub fn save(conn: &Connection, mut emulator: Emulator) -> Result<Emulator> {
    validate(&emulator)?;
    if emulator.id.is_empty() {
        emulator.id = new_id();
    }
    conn.execute(
        "INSERT INTO emulators (id, name, executable_path, emulator_type, platforms, command_template,
           core_name, working_directory, environment, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET name=?2, executable_path=?3, emulator_type=?4, platforms=?5,
           command_template=?6, core_name=?7, working_directory=?8, environment=?9, enabled=?10",
        params![
            emulator.id,
            emulator.name,
            emulator.executable_path,
            emulator.emulator_type,
            serde_json::to_string(&emulator.platforms).unwrap(),
            emulator.command_template,
            emulator.core_name,
            emulator.working_directory,
            serde_json::to_string(&emulator.environment).unwrap(),
            emulator.enabled
        ],
    )?;
    Ok(emulator)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE installations SET emulator_id = NULL WHERE emulator_id = ?1",
        [id],
    )?;
    conn.execute(
        "UPDATE platforms SET default_emulator_id = NULL WHERE default_emulator_id = ?1",
        [id],
    )?;
    conn.execute("DELETE FROM emulators WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Emulator {
        Emulator {
            id: String::new(),
            name: "RetroArch".into(),
            executable_path: "/usr/bin/retroarch".into(),
            emulator_type: "retroarch".into(),
            platforms: vec!["nes".into()],
            command_template: "{executable} -L {corePath} {gamePath}".into(),
            core_name: None,
            working_directory: None,
            environment: Default::default(),
            enabled: true,
        }
    }

    #[test]
    fn validate_accepts_well_formed_config() {
        assert!(validate(&base()).is_ok());
    }

    #[test]
    fn validate_rejects_missing_executable_placeholder() {
        let mut e = base();
        e.command_template = "-L {corePath} {gamePath}".into();
        assert!(validate(&e).is_err());
    }

    #[test]
    fn validate_requires_core_placeholder_for_retroarch() {
        let mut e = base();
        e.command_template = "{executable} {gamePath}".into();
        assert!(validate(&e).is_err());
        e.emulator_type = "standalone".into();
        assert!(validate(&e).is_ok());
    }

    #[test]
    fn validate_rejects_blank_fields() {
        let mut e = base();
        e.executable_path = "  ".into();
        assert!(validate(&e).is_err());
    }
}
