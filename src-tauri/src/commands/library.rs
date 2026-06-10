use super::AppState;
use crate::db::repo::{self, collections, games, platforms, sessions};
use crate::domain::{Collection, Game, GamePatch, LibraryEntry, Platform, PlaySession};
use crate::error::{AppError, Result};
use rusqlite::params;
use serde::Deserialize;
use tauri::State;

#[tauri::command]
pub async fn list_library(state: State<'_, AppState>) -> Result<Vec<LibraryEntry>> {
    state.db.with(games::list_library)
}

#[tauri::command]
pub async fn get_game(state: State<'_, AppState>, game_id: String) -> Result<LibraryEntry> {
    state.db.with(|c| games::entry(c, &game_id))
}

#[tauri::command]
pub async fn update_game(
    state: State<'_, AppState>,
    game_id: String,
    patch: GamePatch,
) -> Result<Game> {
    state.db.with(|c| games::apply_patch(c, &game_id, &patch))
}

#[tauri::command]
pub async fn set_favorite(
    state: State<'_, AppState>,
    game_id: String,
    favorite: bool,
) -> Result<()> {
    state
        .db
        .with(|c| games::set_favorite(c, &game_id, favorite))
}

#[tauri::command]
pub async fn set_hidden(state: State<'_, AppState>, game_id: String, hidden: bool) -> Result<()> {
    state.db.with(|c| games::set_hidden(c, &game_id, hidden))
}

#[tauri::command]
pub async fn delete_game(state: State<'_, AppState>, game_id: String) -> Result<()> {
    state.db.with(|c| games::delete(c, &game_id))
}

#[tauri::command]
pub async fn restore_provider_metadata(
    state: State<'_, AppState>,
    game_id: String,
) -> Result<Game> {
    state
        .db
        .with(|c| games::restore_provider_metadata(c, &game_id))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualGameInput {
    pub title: String,
    pub platform_id: String,
    pub path: Option<String>,
    pub emulator_id: Option<String>,
    pub launch_args: Option<String>,
    pub working_directory: Option<String>,
}

#[tauri::command]
pub async fn add_manual_game(
    state: State<'_, AppState>,
    input: ManualGameInput,
) -> Result<LibraryEntry> {
    if input.title.trim().is_empty() {
        return Err(AppError::Invalid("a title is required".into()));
    }
    state.db.with(|c| {
        platforms::get(c, &input.platform_id)?
            .ok_or_else(|| AppError::Invalid(format!("unknown platform: {}", input.platform_id)))?;
        let game = games::insert(
            c,
            &games::NewGame {
                title: input.title.trim(),
                platform_id: &input.platform_id,
                release_date: None,
                region: None,
            },
        )?;
        c.execute(
            "INSERT INTO installations (id, game_id, source_type, source_id, path, emulator_id, launch_args, working_directory, installed)
             VALUES (?1, ?2, 'manual', NULL, ?3, ?4, ?5, ?6, 1)",
            params![
                repo::new_id(),
                game.id,
                input.path,
                input.emulator_id,
                input.launch_args,
                input.working_directory
            ],
        )?;
        games::entry(c, &game.id)
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationPatch {
    pub installation_id: String,
    pub path: Option<Option<String>>,
    pub emulator_id: Option<Option<String>>,
    pub launch_args: Option<Option<String>>,
    pub working_directory: Option<Option<String>>,
}

#[tauri::command]
pub async fn update_installation(
    state: State<'_, AppState>,
    patch: InstallationPatch,
) -> Result<()> {
    state.db.with(|c| {
        if let Some(path) = &patch.path {
            c.execute(
                "UPDATE installations SET path = ?1 WHERE id = ?2",
                params![path, patch.installation_id],
            )?;
        }
        if let Some(emulator_id) = &patch.emulator_id {
            c.execute(
                "UPDATE installations SET emulator_id = ?1 WHERE id = ?2",
                params![emulator_id, patch.installation_id],
            )?;
        }
        if let Some(launch_args) = &patch.launch_args {
            c.execute(
                "UPDATE installations SET launch_args = ?1 WHERE id = ?2",
                params![launch_args, patch.installation_id],
            )?;
        }
        if let Some(cwd) = &patch.working_directory {
            c.execute(
                "UPDATE installations SET working_directory = ?1 WHERE id = ?2",
                params![cwd, patch.installation_id],
            )?;
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn list_platforms(state: State<'_, AppState>) -> Result<Vec<Platform>> {
    state.db.with(platforms::list)
}

#[tauri::command]
pub async fn set_platform_default_emulator(
    state: State<'_, AppState>,
    platform_id: String,
    emulator_id: Option<String>,
) -> Result<()> {
    state
        .db
        .with(|c| platforms::set_default_emulator(c, &platform_id, emulator_id.as_deref()))
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> Result<Vec<Collection>> {
    state.db.with(collections::list)
}

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, name: String) -> Result<Collection> {
    if name.trim().is_empty() {
        return Err(AppError::Invalid("collection name is required".into()));
    }
    state.db.with(|c| collections::create(c, name.trim()))
}

#[tauri::command]
pub async fn rename_collection(
    state: State<'_, AppState>,
    collection_id: String,
    name: String,
) -> Result<()> {
    state
        .db
        .with(|c| collections::rename(c, &collection_id, name.trim()))
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, collection_id: String) -> Result<()> {
    state.db.with(|c| collections::delete(c, &collection_id))
}

#[tauri::command]
pub async fn add_game_to_collection(
    state: State<'_, AppState>,
    collection_id: String,
    game_id: String,
) -> Result<()> {
    state
        .db
        .with(|c| collections::add_game(c, &collection_id, &game_id))
}

#[tauri::command]
pub async fn remove_game_from_collection(
    state: State<'_, AppState>,
    collection_id: String,
    game_id: String,
) -> Result<()> {
    state
        .db
        .with(|c| collections::remove_game(c, &collection_id, &game_id))
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, AppState>,
    game_id: String,
    limit: Option<u32>,
) -> Result<Vec<PlaySession>> {
    state
        .db
        .with(|c| sessions::for_game(c, &game_id, limit.unwrap_or(25)))
}
