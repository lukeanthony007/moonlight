//! Launch-command construction and child-process supervision.
//!
//! Commands are built as a structured program + argument vector. Placeholders
//! are substituted per-token, so paths containing spaces never get re-split
//! and nothing is ever passed through a shell.

use crate::catalog;
use crate::db::repo::{self, games, sessions};
use crate::db::Db;
use crate::domain::{Emulator, Installation};
use crate::error::{AppError, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    pub environment: HashMap<String, String>,
}

pub struct LaunchContext<'a> {
    pub executable: &'a str,
    pub game_path: Option<&'a str>,
    pub core_path: Option<&'a str>,
    pub extra_args: Option<&'a str>,
}

/// Substitute placeholders inside a single template token.
fn substitute(token: &str, ctx: &LaunchContext) -> Result<Option<Vec<String>>> {
    if token == "{args}" {
        // {args} expands to zero or more whitespace-separated extra args.
        return Ok(Some(
            ctx.extra_args
                .map(|a| a.split_whitespace().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
        ));
    }
    let mut out = token.to_string();
    for (placeholder, value) in [
        ("{executable}", Some(ctx.executable)),
        ("{gamePath}", ctx.game_path),
        ("{corePath}", ctx.core_path),
    ] {
        if out.contains(placeholder) {
            match value {
                Some(v) => out = out.replace(placeholder, v),
                None => {
                    return Err(AppError::Invalid(format!(
                        "command template references {placeholder} but no value is available"
                    )))
                }
            }
        }
    }
    Ok(Some(vec![out]))
}

/// Build a structured launch spec from a command template. The first expanded
/// token becomes the program; the rest become arguments.
pub fn build_spec(
    template: &str,
    ctx: &LaunchContext,
    cwd: Option<&str>,
    env: &HashMap<String, String>,
) -> Result<LaunchSpec> {
    let template = if template.trim().is_empty() {
        "{executable} {gamePath}"
    } else {
        template
    };
    let mut expanded: Vec<String> = Vec::new();
    for token in template.split_whitespace() {
        if let Some(parts) = substitute(token, ctx)? {
            expanded.extend(parts);
        }
    }
    if expanded.is_empty() {
        return Err(AppError::Invalid(
            "command template produced an empty command".into(),
        ));
    }
    let program = expanded.remove(0);
    Ok(LaunchSpec {
        program,
        args: expanded,
        working_directory: cwd.map(|s| s.to_string()),
        environment: env.clone(),
    })
}

/// Resolve the libretro core path for a RetroArch launch.
pub fn resolve_core_path(
    conn: &rusqlite::Connection,
    emulator: &Emulator,
    platform_id: &str,
) -> Result<Option<String>> {
    if emulator.emulator_type != "retroarch" {
        return Ok(None);
    }
    // Per-platform core mapping from settings wins over the emulator default.
    if let Some(map) = repo::settings::get(conn, "retroarch.coreMap")? {
        if let Some(path) = map.get(platform_id).and_then(|v| v.as_str()) {
            return Ok(Some(path.to_string()));
        }
    }
    if let Some(core) = &emulator.core_name {
        return Ok(Some(core.clone()));
    }
    Err(AppError::Launch(format!(
        "no RetroArch core configured for platform '{platform_id}'. Assign one in Settings → Emulators."
    )))
}

/// Build the full launch spec for an installation, validating files exist.
pub fn prepare_launch(
    conn: &rusqlite::Connection,
    install: &Installation,
    platform_id: &str,
) -> Result<LaunchSpec> {
    if install.source_type == "steam" {
        let appid = install
            .source_id
            .as_deref()
            .ok_or_else(|| AppError::Launch("Steam installation has no app id".into()))?;
        let steam = crate::steam::steam_command(conn)?;
        return Ok(LaunchSpec {
            program: steam,
            args: vec![format!("steam://rungameid/{appid}")],
            working_directory: None,
            environment: HashMap::new(),
        });
    }

    let game_path = install.path.as_deref();
    if let Some(p) = game_path {
        if !Path::new(p).exists() {
            return Err(AppError::Launch(format!("game file not found: {p}")));
        }
    }

    // Native executables and scripts launch directly without an emulator.
    let emulator = match &install.emulator_id {
        Some(id) => Some(repo::emulators::get(conn, id).map_err(|_| {
            AppError::Launch("the emulator configured for this game no longer exists".into())
        })?),
        None => None,
    };

    match emulator {
        None => {
            let path = game_path
                .ok_or_else(|| AppError::Launch("no file path configured for this game".into()))?;
            let args = install
                .launch_args
                .as_deref()
                .map(|a| a.split_whitespace().map(|s| s.to_string()).collect())
                .unwrap_or_default();
            Ok(LaunchSpec {
                program: path.to_string(),
                args,
                working_directory: install.working_directory.clone(),
                environment: HashMap::new(),
            })
        }
        Some(emulator) => {
            if !emulator.enabled {
                return Err(AppError::Launch(format!(
                    "emulator '{}' is disabled",
                    emulator.name
                )));
            }
            if !Path::new(&emulator.executable_path).exists() {
                return Err(AppError::Launch(format!(
                    "emulator executable not found: {}",
                    emulator.executable_path
                )));
            }
            let core_path = resolve_core_path(conn, &emulator, platform_id)?;
            if let Some(core) = &core_path {
                if !Path::new(core).exists() {
                    return Err(AppError::Launch(format!(
                        "RetroArch core not found: {core}"
                    )));
                }
            }
            let ctx = LaunchContext {
                executable: &emulator.executable_path,
                game_path,
                core_path: core_path.as_deref(),
                extra_args: install.launch_args.as_deref(),
            };
            let cwd = install
                .working_directory
                .as_deref()
                .or(emulator.working_directory.as_deref());
            build_spec(&emulator.command_template, &ctx, cwd, &emulator.environment)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningGame {
    pub game_id: String,
    pub session_id: String,
    pub started_at: String,
}

#[derive(Default)]
pub struct RunningSessions(pub Mutex<HashMap<String, RunningGame>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndedPayload {
    pub game_id: String,
    pub session_id: String,
    pub duration_seconds: i64,
    pub exit_status: String,
}

/// Spawn the process and supervise it on a background thread. Records the
/// play session and updates playtime when the child exits.
pub fn launch_and_track(
    app: tauri::AppHandle,
    db: Arc<Db>,
    game_id: String,
    spec: LaunchSpec,
    detached: bool,
) -> Result<RunningGame> {
    {
        let running = app.state::<RunningSessions>();
        if running.0.lock().unwrap().contains_key(&game_id) {
            return Err(AppError::Launch("this game is already running".into()));
        }
    }

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = &spec.working_directory {
        command.current_dir(cwd);
    }
    for (key, value) in &spec.environment {
        command.env(key, value);
    }

    tracing::info!(game = %game_id, program = %spec.program, args = ?spec.args, "launching game");
    let mut child = command.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            AppError::Launch(format!("executable not found: {}", spec.program))
        }
        std::io::ErrorKind::PermissionDenied => {
            AppError::Launch(format!("permission denied executing: {}", spec.program))
        }
        _ => AppError::Launch(format!("failed to start process: {e}")),
    })?;

    let session = db.with(|c| sessions::start(c, &game_id))?;
    let running_game = RunningGame {
        game_id: game_id.clone(),
        session_id: session.id.clone(),
        started_at: session.started_at.clone(),
    };
    app.state::<RunningSessions>()
        .0
        .lock()
        .unwrap()
        .insert(game_id.clone(), running_game.clone());
    let _ = app.emit("session-started", &running_game);

    let session_id = session.id.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let status = child.wait();
        let duration = started.elapsed().as_secs() as i64;
        let exit_status = match (&status, detached) {
            (_, true) => "detached".to_string(),
            (Ok(s), _) if s.success() => "ok".to_string(),
            (Ok(s), _) => format!(
                "error({})",
                s.code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "signal".into())
            ),
            (Err(e), _) => format!("error({e})"),
        };
        tracing::info!(game = %game_id, duration, status = %exit_status, "game session ended");

        let result = db.with(|c| {
            sessions::end(c, &session_id, duration, &exit_status)?;
            // Detached launchers (e.g. Steam) exit immediately; still bump
            // last-played but don't pollute playtime with launcher runtime.
            let counted = if detached { 0 } else { duration };
            games::record_play(c, &game_id, &repo::now(), counted)?;
            Ok(())
        });
        if let Err(e) = result {
            tracing::error!(error = %e, "failed to record session end");
        }
        app.state::<RunningSessions>()
            .0
            .lock()
            .unwrap()
            .remove(&game_id);
        let _ = app.emit(
            "session-ended",
            SessionEndedPayload {
                game_id: game_id.clone(),
                session_id: session_id.clone(),
                duration_seconds: duration,
                exit_status,
            },
        );
    });

    Ok(running_game)
}

/// Validate an emulator config by launching it with no game (then letting the
/// user close it), or just verifying the binary exists and is executable.
pub fn test_emulator(emulator: &Emulator) -> Result<String> {
    let path = Path::new(&emulator.executable_path);
    if !path.exists() {
        return Err(AppError::Launch(format!(
            "executable not found: {}",
            emulator.executable_path
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = path.metadata()?.permissions().mode();
        if mode & 0o111 == 0 {
            return Err(AppError::Launch(format!(
                "file is not executable: {}",
                emulator.executable_path
            )));
        }
    }
    // Build a dry-run spec to surface template errors early.
    let ctx = LaunchContext {
        executable: &emulator.executable_path,
        game_path: Some("<game>"),
        core_path: Some("<core>"),
        extra_args: None,
    };
    let spec = build_spec(
        &emulator.command_template,
        &ctx,
        None,
        &emulator.environment,
    )?;
    Ok(format!("{} {}", spec.program, spec.args.join(" ")))
}

/// Re-export used by command layer for core detection responses.
pub use catalog::core_platform_hints;

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>() -> LaunchContext<'a> {
        LaunchContext {
            executable: "/usr/bin/retroarch",
            game_path: Some("/roms/Super Mario 64 (USA).z64"),
            core_path: Some("/cores/mupen64plus_next_libretro.so"),
            extra_args: None,
        }
    }

    #[test]
    fn builds_retroarch_command_with_spaced_path_as_single_arg() {
        let spec = build_spec(
            "{executable} -L {corePath} {gamePath}",
            &ctx(),
            None,
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(spec.program, "/usr/bin/retroarch");
        assert_eq!(
            spec.args,
            vec![
                "-L",
                "/cores/mupen64plus_next_libretro.so",
                "/roms/Super Mario 64 (USA).z64"
            ]
        );
    }

    #[test]
    fn expands_args_placeholder_to_multiple_arguments() {
        let c = LaunchContext {
            extra_args: Some("--fullscreen --config special.cfg"),
            ..ctx()
        };
        let spec = build_spec("{executable} {args} {gamePath}", &c, None, &HashMap::new()).unwrap();
        assert_eq!(
            spec.args,
            vec![
                "--fullscreen",
                "--config",
                "special.cfg",
                "/roms/Super Mario 64 (USA).z64"
            ]
        );
    }

    #[test]
    fn empty_args_placeholder_expands_to_nothing() {
        let spec = build_spec(
            "{executable} {args} {gamePath}",
            &ctx(),
            None,
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(spec.args, vec!["/roms/Super Mario 64 (USA).z64"]);
    }

    #[test]
    fn missing_core_value_is_an_error() {
        let c = LaunchContext {
            core_path: None,
            ..ctx()
        };
        let err = build_spec(
            "{executable} -L {corePath} {gamePath}",
            &c,
            None,
            &HashMap::new(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("{corePath}"));
    }

    #[test]
    fn empty_template_defaults_to_executable_and_game() {
        let spec = build_spec("  ", &ctx(), None, &HashMap::new()).unwrap();
        assert_eq!(spec.program, "/usr/bin/retroarch");
        assert_eq!(spec.args, vec!["/roms/Super Mario 64 (USA).z64"]);
    }

    #[test]
    fn placeholder_embedded_in_flag_token_is_substituted() {
        let spec = build_spec(
            "{executable} --rom={gamePath}",
            &ctx(),
            None,
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(spec.args, vec!["--rom=/roms/Super Mario 64 (USA).z64"]);
    }

    #[test]
    fn working_directory_and_env_carry_through() {
        let mut env = HashMap::new();
        env.insert("WINEPREFIX".to_string(), "/wine".to_string());
        let spec = build_spec("{executable} {gamePath}", &ctx(), Some("/work"), &env).unwrap();
        assert_eq!(spec.working_directory.as_deref(), Some("/work"));
        assert_eq!(
            spec.environment.get("WINEPREFIX").map(|s| s.as_str()),
            Some("/wine")
        );
    }
}
