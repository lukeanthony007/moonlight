pub mod artwork_store;
pub mod catalog;
pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod launch;
pub mod metadata;
pub mod paths;
pub mod retroarch;
pub mod scan;
pub mod steam;

use commands::AppState;
use db::repo::{platforms, sessions, settings};
use db::Db;
use launch::RunningSessions;
use paths::AppPaths;
use scan::ScanRegistry;
use std::sync::Arc;
use tauri::Manager;

fn init_logging(paths: &AppPaths) -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = tracing_appender::rolling::daily(&paths.log_dir, "moonlight.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,moonlight_lib=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer),
        )
        .init();
    guard
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = AppPaths::resolve(app.handle())?;
            let guard = init_logging(&paths);
            app.manage(guard);
            tracing::info!(data_dir = %paths.data_dir.display(), "starting Moonlight");

            let db = Arc::new(Db::open(&paths.db_path)?);
            db.with(|c| {
                for platform in catalog::builtin_platforms() {
                    platforms::ensure(c, &platform)?;
                }
                let dangling = sessions::close_dangling(c)?;
                if dangling > 0 {
                    tracing::warn!(
                        count = dangling,
                        "closed dangling play sessions from previous run"
                    );
                }
                Ok(())
            })?;

            app.manage(AppState {
                db: db.clone(),
                paths: paths.clone(),
            });
            app.manage(RunningSessions::default());
            app.manage(ScanRegistry::default());

            // Optional automatic scan on startup.
            let scan_on_startup = db
                .with(|c| settings::get_bool(c, "general.scanOnStartup", false))
                .unwrap_or(false);
            if scan_on_startup {
                let handle = app.handle().clone();
                let db = db.clone();
                let artwork_dir = paths.artwork_dir.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    use tauri::Emitter;
                    let scan_id = uuid::Uuid::new_v4().to_string();
                    let cancel = handle.state::<ScanRegistry>().begin(&scan_id);
                    let report = scan::run_scan(
                        &db,
                        &artwork_dir,
                        scan::ScanScope::All,
                        scan_id.clone(),
                        cancel,
                        Some(&handle),
                    );
                    handle
                        .state::<ScanRegistry>()
                        .finish(&scan_id, report.clone());
                    let _ = handle.emit("scan-complete", &report);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library::list_library,
            commands::library::get_game,
            commands::library::update_game,
            commands::library::set_favorite,
            commands::library::set_hidden,
            commands::library::delete_game,
            commands::library::restore_provider_metadata,
            commands::library::add_manual_game,
            commands::library::update_installation,
            commands::library::list_platforms,
            commands::library::set_platform_default_emulator,
            commands::library::list_collections,
            commands::library::create_collection,
            commands::library::rename_collection,
            commands::library::delete_collection,
            commands::library::add_game_to_collection,
            commands::library::remove_game_from_collection,
            commands::library::list_sessions,
            commands::emulators::list_emulators,
            commands::emulators::save_emulator,
            commands::emulators::delete_emulator,
            commands::emulators::test_emulator_config,
            commands::emulators::get_emulator_presets,
            commands::emulators::detect_retroarch_cores,
            commands::emulators::detect_emulator_executable,
            commands::emulators::list_rom_directories,
            commands::emulators::save_rom_directory,
            commands::emulators::delete_rom_directory,
            commands::launching::launch_game,
            commands::launching::get_running_games,
            commands::scanning::start_scan,
            commands::scanning::cancel_scan,
            commands::scanning::get_last_scan_report,
            commands::meta::get_provider_statuses,
            commands::meta::search_metadata,
            commands::meta::get_artwork_candidates,
            commands::meta::apply_provider_match,
            commands::meta::list_artwork,
            commands::meta::import_artwork_file,
            commands::meta::download_artwork,
            commands::meta::select_artwork,
            commands::meta::delete_artwork,
            commands::settings_cmd::get_settings,
            commands::settings_cmd::set_setting,
            commands::settings_cmd::get_app_info,
            commands::settings_cmd::complete_onboarding,
            commands::settings_cmd::backup_database,
            commands::settings_cmd::export_library,
            commands::diagnostics::get_diagnostics,
            commands::diagnostics::read_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
