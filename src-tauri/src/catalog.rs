//! Built-in platform definitions and emulator presets. These seed the
//! database on first run; users can edit everything afterwards.

use crate::domain::Platform;
use serde::Serialize;

fn platform(
    id: &str,
    name: &str,
    short_name: &str,
    manufacturer: Option<&str>,
    extensions: &[&str],
    sort_order: i64,
) -> Platform {
    Platform {
        id: id.into(),
        name: name.into(),
        short_name: short_name.into(),
        manufacturer: manufacturer.map(|m| m.to_string()),
        default_emulator_id: None,
        extensions: extensions.iter().map(|e| e.to_string()).collect(),
        sort_order,
    }
}

pub fn builtin_platforms() -> Vec<Platform> {
    vec![
        platform("steam", "Steam", "Steam", Some("Valve"), &[], 0),
        platform(
            "windows",
            "Windows",
            "PC",
            Some("Microsoft"),
            &["exe", "bat", "lnk"],
            1,
        ),
        // Native Linux games (Lutris's "linux" runner): shell launchers,
        // AppImages and itch.io-style binaries. They launch directly with no
        // emulator; Windows games on Linux go through a Wine/Proton runner.
        platform(
            "linux",
            "Linux",
            "Linux",
            None,
            &["sh", "appimage", "run", "x86_64"],
            2,
        ),
        platform(
            "nes",
            "Nintendo Entertainment System",
            "NES",
            Some("Nintendo"),
            &["nes", "fds", "unf", "zip", "7z"],
            10,
        ),
        platform(
            "snes",
            "Super Nintendo",
            "SNES",
            Some("Nintendo"),
            &["sfc", "smc", "fig", "zip", "7z"],
            11,
        ),
        platform(
            "n64",
            "Nintendo 64",
            "N64",
            Some("Nintendo"),
            &["n64", "z64", "v64", "zip", "7z"],
            12,
        ),
        platform(
            "gamecube",
            "GameCube",
            "GCN",
            Some("Nintendo"),
            &["iso", "gcm", "gcz", "rvz", "ciso"],
            13,
        ),
        platform(
            "wii",
            "Wii",
            "Wii",
            Some("Nintendo"),
            &["iso", "wbfs", "rvz", "wad"],
            14,
        ),
        platform(
            "switch",
            "Nintendo Switch",
            "Switch",
            Some("Nintendo"),
            &["nsp", "xci", "nca", "nro"],
            15,
        ),
        platform(
            "gb",
            "Game Boy",
            "GB",
            Some("Nintendo"),
            &["gb", "zip", "7z"],
            16,
        ),
        platform(
            "gbc",
            "Game Boy Color",
            "GBC",
            Some("Nintendo"),
            &["gbc", "zip", "7z"],
            17,
        ),
        platform(
            "gba",
            "Game Boy Advance",
            "GBA",
            Some("Nintendo"),
            &["gba", "zip", "7z"],
            18,
        ),
        platform(
            "nds",
            "Nintendo DS",
            "NDS",
            Some("Nintendo"),
            &["nds", "zip", "7z"],
            19,
        ),
        platform(
            "psx",
            "PlayStation",
            "PS1",
            Some("Sony"),
            &["cue", "bin", "chd", "pbp", "iso", "m3u"],
            20,
        ),
        platform(
            "ps2",
            "PlayStation 2",
            "PS2",
            Some("Sony"),
            &["iso", "chd", "cso", "bin", "mdf", "nrg"],
            21,
        ),
        platform(
            "ps3",
            "PlayStation 3",
            "PS3",
            Some("Sony"),
            &["iso", "pkg"],
            22,
        ),
        platform(
            "psp",
            "PlayStation Portable",
            "PSP",
            Some("Sony"),
            &["iso", "cso", "chd", "pbp"],
            23,
        ),
        platform(
            "genesis",
            "Sega Genesis",
            "Genesis",
            Some("Sega"),
            &["md", "gen", "bin", "smd", "zip", "7z"],
            30,
        ),
        platform(
            "mastersystem",
            "Sega Master System",
            "SMS",
            Some("Sega"),
            &["sms", "zip", "7z"],
            31,
        ),
        platform(
            "saturn",
            "Sega Saturn",
            "Saturn",
            Some("Sega"),
            &["cue", "chd", "iso", "m3u"],
            32,
        ),
        platform(
            "dreamcast",
            "Sega Dreamcast",
            "DC",
            Some("Sega"),
            &["gdi", "chd", "cdi", "m3u"],
            33,
        ),
        platform(
            "arcade",
            "Arcade",
            "Arcade",
            None,
            &["zip", "chd", "7z"],
            40,
        ),
    ]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorPreset {
    pub id: String,
    pub name: String,
    pub emulator_type: String,
    pub platforms: Vec<String>,
    pub command_template: String,
    pub executable_hints: Vec<String>,
    pub extensions: Vec<String>,
}

fn preset(
    id: &str,
    name: &str,
    emulator_type: &str,
    platforms: &[&str],
    command_template: &str,
    executable_hints: &[&str],
    extensions: &[&str],
) -> EmulatorPreset {
    EmulatorPreset {
        id: id.into(),
        name: name.into(),
        emulator_type: emulator_type.into(),
        platforms: platforms.iter().map(|p| p.to_string()).collect(),
        command_template: command_template.into(),
        executable_hints: executable_hints.iter().map(|h| h.to_string()).collect(),
        extensions: extensions.iter().map(|e| e.to_string()).collect(),
    }
}

pub fn emulator_presets() -> Vec<EmulatorPreset> {
    vec![
        preset(
            "retroarch",
            "RetroArch",
            "retroarch",
            &[
                "nes",
                "snes",
                "n64",
                "gb",
                "gbc",
                "gba",
                "nds",
                "psx",
                "genesis",
                "mastersystem",
                "saturn",
                "dreamcast",
                "arcade",
                "psp",
            ],
            "{executable} -L {corePath} {gamePath}",
            &[
                "/usr/bin/retroarch",
                "/usr/local/bin/retroarch",
                "/var/lib/flatpak/exports/bin/org.libretro.RetroArch",
                "C:\\RetroArch-Win64\\retroarch.exe",
                "/Applications/RetroArch.app/Contents/MacOS/RetroArch",
            ],
            &[],
        ),
        preset(
            "dolphin",
            "Dolphin",
            "standalone",
            &["gamecube", "wii"],
            "{executable} -b -e {gamePath}",
            &[
                "/usr/bin/dolphin-emu",
                "/var/lib/flatpak/exports/bin/org.DolphinEmu.dolphin-emu",
                "C:\\Program Files\\Dolphin\\Dolphin.exe",
                "/Applications/Dolphin.app/Contents/MacOS/Dolphin",
            ],
            &["iso", "gcm", "gcz", "rvz", "wbfs", "ciso", "wad"],
        ),
        preset(
            "pcsx2",
            "PCSX2",
            "standalone",
            &["ps2"],
            "{executable} -nogui {gamePath}",
            &[
                "/usr/bin/pcsx2-qt",
                "/usr/bin/pcsx2",
                "/var/lib/flatpak/exports/bin/net.pcsx2.PCSX2",
                "C:\\Program Files\\PCSX2\\pcsx2-qt.exe",
            ],
            &["iso", "chd", "cso", "bin"],
        ),
        preset(
            "ryujinx",
            "Ryujinx",
            "standalone",
            &["switch"],
            "{executable} {gamePath}",
            &[
                "/usr/bin/ryujinx",
                "/var/lib/flatpak/exports/bin/org.ryujinx.Ryujinx",
                "C:\\Program Files\\Ryujinx\\Ryujinx.exe",
            ],
            &["nsp", "xci", "nca", "nro"],
        ),
        preset(
            "ppsspp",
            "PPSSPP",
            "standalone",
            &["psp"],
            "{executable} {gamePath}",
            &[
                "/usr/bin/PPSSPPQt",
                "/usr/bin/ppsspp",
                "/var/lib/flatpak/exports/bin/org.ppsspp.PPSSPP",
            ],
            &["iso", "cso", "chd", "pbp"],
        ),
        preset(
            "duckstation",
            "DuckStation",
            "standalone",
            &["psx"],
            "{executable} -batch {gamePath}",
            &[
                "/usr/bin/duckstation-qt",
                "/var/lib/flatpak/exports/bin/org.duckstation.DuckStation",
            ],
            &["cue", "bin", "chd", "pbp", "iso", "m3u"],
        ),
        // Windows-game runners (Lutris-style): the "platform" stays Windows,
        // the runner decides how the exe is launched on this OS.
        preset(
            "wine",
            "Wine",
            "standalone",
            &["windows"],
            "{executable} {gamePath}",
            &[
                "/usr/bin/wine",
                "/usr/local/bin/wine",
                "/var/lib/flatpak/exports/bin/org.winehq.Wine",
            ],
            &["exe", "bat", "msi"],
        ),
        preset(
            "umu",
            "Proton (umu)",
            "standalone",
            &["windows"],
            "{executable} {gamePath}",
            &["/usr/bin/umu-run", "/usr/local/bin/umu-run"],
            &["exe", "bat", "msi"],
        ),
        preset(
            "custom",
            "Custom emulator",
            "custom",
            &[],
            "{executable} {gamePath}",
            &[],
            &[],
        ),
    ]
}

/// Map well-known libretro core file stems to platform ids.
pub fn core_platform_hints() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fceumm", "nes"),
        ("nestopia", "nes"),
        ("mesen", "nes"),
        ("snes9x", "snes"),
        ("bsnes", "snes"),
        ("mupen64plus_next", "n64"),
        ("parallel_n64", "n64"),
        ("dolphin", "gamecube"),
        ("gambatte", "gb"),
        ("sameboy", "gb"),
        ("mgba", "gba"),
        ("vba_next", "gba"),
        ("melonds", "nds"),
        ("desmume", "nds"),
        ("swanstation", "psx"),
        ("beetle_psx", "psx"),
        ("pcsx_rearmed", "psx"),
        ("ppsspp", "psp"),
        ("genesis_plus_gx", "genesis"),
        ("picodrive", "genesis"),
        ("beetle_saturn", "saturn"),
        ("flycast", "dreamcast"),
        ("mame", "arcade"),
        ("fbneo", "arcade"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lutris-style mapping: a native Linux platform exists, and Windows
    /// games get Wine/Proton runner presets instead of a separate platform.
    #[test]
    fn linux_platform_and_windows_runners_exist() {
        let platforms = builtin_platforms();
        let linux = platforms
            .iter()
            .find(|p| p.id == "linux")
            .expect("linux platform");
        assert!(linux.extensions.contains(&"sh".to_string()));
        assert!(linux.extensions.contains(&"appimage".to_string()));

        let windows = platforms.iter().find(|p| p.id == "windows").unwrap();
        assert!(
            !windows.extensions.contains(&"sh".to_string()),
            ".sh belongs to linux now"
        );

        let presets = emulator_presets();
        for runner in ["wine", "umu"] {
            let p = presets.iter().find(|p| p.id == runner).expect(runner);
            assert_eq!(p.platforms, vec!["windows"], "{runner} runs Windows games");
            assert!(p.command_template.contains("{executable}"));
        }
    }
}
