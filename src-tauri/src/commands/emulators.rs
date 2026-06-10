use super::AppState;
use crate::catalog::{emulator_presets, EmulatorPreset};
use crate::db::repo::{emulators, rom_dirs};
use crate::domain::{Emulator, RomDirectory};
use crate::error::Result;
use crate::retroarch::{self, DetectedCore};
use tauri::State;

#[tauri::command]
pub async fn list_emulators(state: State<'_, AppState>) -> Result<Vec<Emulator>> {
    state.db.with(emulators::list)
}

#[tauri::command]
pub async fn save_emulator(state: State<'_, AppState>, emulator: Emulator) -> Result<Emulator> {
    state.db.with(|c| emulators::save(c, emulator))
}

#[tauri::command]
pub async fn delete_emulator(state: State<'_, AppState>, emulator_id: String) -> Result<()> {
    state.db.with(|c| emulators::delete(c, &emulator_id))
}

/// Dry-run validation of an emulator configuration. Returns the command line
/// that would be executed for a hypothetical game.
#[tauri::command]
pub async fn test_emulator_config(emulator: Emulator) -> Result<String> {
    emulators::validate(&emulator)?;
    crate::launch::test_emulator(&emulator)
}

#[tauri::command]
pub fn get_emulator_presets() -> Vec<EmulatorPreset> {
    emulator_presets()
}

#[tauri::command]
pub async fn detect_retroarch_cores(custom_dir: Option<String>) -> Result<Vec<DetectedCore>> {
    Ok(retroarch::detect_cores(custom_dir.as_deref()))
}

#[tauri::command]
pub async fn detect_emulator_executable(preset_id: String) -> Result<Option<String>> {
    if preset_id == "retroarch" {
        return Ok(retroarch::detect_executable());
    }
    let preset = emulator_presets().into_iter().find(|p| p.id == preset_id);
    Ok(preset.and_then(|p| {
        p.executable_hints
            .iter()
            .find(|hint| std::path::Path::new(hint).exists())
            .cloned()
    }))
}

#[tauri::command]
pub async fn list_rom_directories(state: State<'_, AppState>) -> Result<Vec<RomDirectory>> {
    state.db.with(rom_dirs::list)
}

#[tauri::command]
pub async fn save_rom_directory(
    state: State<'_, AppState>,
    directory: RomDirectory,
) -> Result<RomDirectory> {
    state.db.with(|c| rom_dirs::save(c, directory))
}

#[tauri::command]
pub async fn delete_rom_directory(state: State<'_, AppState>, directory_id: String) -> Result<()> {
    state.db.with(|c| rom_dirs::delete(c, &directory_id))
}
