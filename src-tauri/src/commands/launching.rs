use super::AppState;
use crate::db::repo::games;
use crate::error::{AppError, Result};
use crate::launch::{self, RunningGame, RunningSessions};
use tauri::State;

#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    game_id: String,
) -> Result<RunningGame> {
    let (install, platform_id) = state.db.with(|c| {
        let game = games::get(c, &game_id)?;
        let installs = games::installations_for(c, &game_id)?;
        let install = installs
            .iter()
            .find(|i| i.installed)
            .or(installs.first())
            .cloned()
            .ok_or_else(|| AppError::Launch("this game has no installation configured".into()))?;
        Ok((install, game.platform_id))
    })?;

    let spec = state
        .db
        .with(|c| launch::prepare_launch(c, &install, &platform_id))?;
    let detached = install.source_type == "steam";
    launch::launch_and_track(app, state.db.clone(), game_id, spec, detached)
}

#[tauri::command]
pub fn get_running_games(running: State<'_, RunningSessions>) -> Vec<RunningGame> {
    running.0.lock().unwrap().values().cloned().collect()
}
