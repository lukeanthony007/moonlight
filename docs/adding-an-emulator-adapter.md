# Adding an emulator adapter

Moonlight treats emulators as data, not code: an emulator is a row in the `emulators` table with a
command template, so most "adapters" are just a preset. Two extension points exist.

## 1. Add a preset (most emulators)

Edit `src-tauri/src/catalog.rs` → `emulator_presets()` and add an entry:

```rust
preset(
    "melonds",                     // stable preset id
    "melonDS",                     // display name
    "standalone",                  // "retroarch" | "standalone" | "custom"
    &["nds"],                      // platform ids it emulates (see builtin_platforms())
    "{executable} -f {gamePath}",  // launch template
    &[                             // executable auto-detection hints, checked in order
        "/usr/bin/melonDS",
        "/var/lib/flatpak/exports/bin/net.kuribo64.melonDS",
        "C:\\Program Files\\melonDS\\melonDS.exe",
    ],
    &["nds"],                      // file extensions to scan
),
```

That's everything: the preset appears in the "Connect emulator" wizard, the executable hints power
auto-detection (`detect_emulator_executable`), and the template is validated by
`db::repo::emulators::validate` (must contain `{executable}`; RetroArch-type templates must contain
`{corePath}`).

### Template placeholders

Templates are split into whitespace-separated tokens; placeholders are substituted **per token**, so
paths containing spaces stay a single argument and nothing ever passes through a shell
(`launch::build_spec`):

| Placeholder | Meaning |
| --- | --- |
| `{executable}` | the emulator binary (first token becomes the program) |
| `{gamePath}` | the resolved game file |
| `{corePath}` | the libretro core (RetroArch-type emulators) |
| `{args}` | expands to the installation's per-game extra arguments (zero or more tokens) |

Placeholders may be embedded inside flags (`--rom={gamePath}`).

If a new platform is needed, add it to `builtin_platforms()` in the same file — it is seeded with
`INSERT OR IGNORE`, so existing user databases pick it up on next launch without a migration.

## 2. Launch behavior that isn't template-shaped

Sources with special launch semantics (like Steam's `steam://rungameid/{appid}`) are handled in
`launch::prepare_launch` by matching on the installation's `source_type`. If you integrate a launcher
that needs custom resolution (e.g. a storefront with its own URI scheme):

1. Give its installations a distinct `source_type` when importing (see `scan.rs`).
2. Add a branch in `prepare_launch` that builds a `LaunchSpec { program, args, working_directory,
   environment }` for it.
3. If the spawned process detaches immediately (launcher hands off to another process), pass
   `detached = true` to `launch_and_track` (see `commands/launching.rs`) so launcher runtime is not
   counted as playtime.

Add tests next to the existing ones in `launch.rs` — `build_spec` and `prepare_launch` are pure enough
to test against an in-memory database.
