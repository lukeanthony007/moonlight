import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Database,
  FolderOpen,
  Gamepad2,
  HardDrive,
  Image,
  Info,
  Keyboard,
  Layers,
  Palette,
  Pencil,
  Plus,
  RefreshCw,
  Settings as SettingsIcon,
  Trash2,
} from "lucide-react";
import * as api from "@/lib/api";
import type { Diagnostics, Emulator, RomDirectory } from "@/lib/types";
import { cn, formatBytes } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge, Label, SectionTitle, SettingRow, Spinner, Switch } from "@/components/ui/misc";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { EmulatorWizard } from "@/components/dialogs/EmulatorWizard";
import { useLibraryStore } from "@/stores/libraryStore";
import { useScanStore } from "@/stores/scanStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useUiStore, type SettingsSection } from "@/stores/uiStore";

const SECTIONS: { id: SettingsSection; label: string; icon: React.ReactNode }[] = [
  { id: "general", label: "General", icon: <SettingsIcon /> },
  { id: "libraries", label: "Game Libraries", icon: <HardDrive /> },
  { id: "emulators", label: "Emulators", icon: <Gamepad2 /> },
  { id: "platforms", label: "Platforms", icon: <Layers /> },
  { id: "providers", label: "Metadata Providers", icon: <Database /> },
  { id: "artwork", label: "Artwork Storage", icon: <Image /> },
  { id: "appearance", label: "Appearance", icon: <Palette /> },
  { id: "input", label: "Input", icon: <Keyboard /> },
  { id: "importExport", label: "Import & Export", icon: <FolderOpen /> },
  { id: "diagnostics", label: "Diagnostics", icon: <Info /> },
];

export function Settings({ section }: { section: SettingsSection }) {
  const navigate = useUiStore((s) => s.navigate);

  return (
    <div className="flex h-full gap-6 px-8 pb-8 pt-7">
      <nav aria-label="Settings sections" className="w-56 shrink-0">
        <h1 className="mb-5 text-2xl font-bold tracking-tight">Settings</h1>
        <div className="space-y-0.5">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              className={cn(
                "focusable flex w-full items-center gap-3 rounded-xl px-3.5 py-2.5 text-left text-sm transition-colors cursor-pointer [&>svg]:size-4",
                section === s.id ? "glass-strong text-ink" : "text-ink-dim hover:bg-glass hover:text-ink",
              )}
              aria-current={section === s.id ? "page" : undefined}
              onClick={() => navigate({ name: "settings", section: s.id })}
            >
              {s.icon}
              {s.label}
            </button>
          ))}
        </div>
      </nav>

      <div className="glass card-shadow min-w-0 flex-1 overflow-y-auto rounded-panel p-7">
        {section === "general" && <GeneralSection />}
        {section === "libraries" && <LibrariesSection />}
        {section === "emulators" && <EmulatorsSection />}
        {section === "platforms" && <PlatformsSection />}
        {section === "providers" && <ProvidersSection />}
        {section === "artwork" && <ArtworkSection />}
        {section === "appearance" && <AppearanceSection />}
        {section === "input" && <InputSection />}
        {section === "importExport" && <ImportExportSection />}
        {section === "diagnostics" && <DiagnosticsSection />}
      </div>
    </div>
  );
}

function GeneralSection() {
  const settings = useSettingsStore();
  return (
    <div>
      <SectionTitle title="General" description="Library behavior and startup options." />
      <SettingRow label="Scan on startup" description="Automatically scan all sources when Moonlight opens.">
        <Switch
          checked={settings.get("general.scanOnStartup", false)}
          onCheckedChange={(v) => void settings.set("general.scanOnStartup", v)}
          aria-label="Scan on startup"
        />
      </SettingRow>
      <SettingRow label="Default view" description="Layout used for library pages.">
        <Select
          value={settings.get("general.defaultView", "grid")}
          onValueChange={(v) => void settings.set("general.defaultView", v)}
        >
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="grid">Grid</SelectItem>
            <SelectItem value="list">List</SelectItem>
          </SelectContent>
        </Select>
      </SettingRow>
    </div>
  );
}

function LibrariesSection() {
  const settings = useSettingsStore();
  const romDirectories = useLibraryStore((s) => s.romDirectories);
  const platforms = useLibraryStore((s) => s.platforms);
  const refresh = useLibraryStore((s) => s.refresh);
  const startScan = useScanStore((s) => s.start);
  const activeScanId = useScanStore((s) => s.activeScanId);
  const [steamPath, setSteamPath] = useState(settings.get("steam.path", ""));

  useEffect(() => {
    setSteamPath(settings.get("steam.path", ""));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.loaded]);

  async function addDirectory() {
    const selected = await open({ directory: true, multiple: false, title: "Select a game directory" });
    if (typeof selected !== "string") return;
    await api.saveRomDirectory({
      id: "",
      path: selected,
      platformId: platforms.find((p) => p.id !== "steam")?.id ?? "windows",
      emulatorId: null,
      enabled: true,
    });
    await refresh();
  }

  return (
    <div>
      <SectionTitle title="Game Libraries" description="Sources Moonlight scans for games." />

      <h3 className="mb-2 mt-2 text-sm font-semibold">Steam</h3>
      <div className="rounded-xl glass p-4">
        <div className="flex items-end gap-3">
          <div className="flex-1 space-y-1.5">
            <Label htmlFor="steam-path">Steam installation path (leave empty to auto-detect)</Label>
            <Input
              id="steam-path"
              value={steamPath}
              placeholder="~/.local/share/Steam"
              onChange={(e) => setSteamPath(e.target.value)}
              onBlur={() => void settings.set("steam.path", steamPath)}
            />
          </div>
          <Button
            variant="glass"
            disabled={activeScanId !== null}
            onClick={() => startScan({ kind: "steam" })}
          >
            <RefreshCw /> Import Steam games
          </Button>
        </div>
        <p className="mt-2 text-xs text-ink-faint">
          Imports installed games and cached artwork from the Steam client. No credentials required.
        </p>
      </div>

      <div className="mb-2 mt-7 flex items-center justify-between">
        <h3 className="text-sm font-semibold">ROM directories</h3>
        <Button variant="glass" size="sm" onClick={addDirectory}>
          <Plus /> Add directory
        </Button>
      </div>
      {romDirectories.length === 0 && (
        <p className="text-sm text-ink-faint">
          No directories yet. Add one here or via Settings → Emulators → Connect emulator.
        </p>
      )}
      <div className="space-y-2">
        {romDirectories.map((dir) => (
          <RomDirRow key={dir.id} dir={dir} />
        ))}
      </div>
    </div>
  );
}

function RomDirRow({ dir }: { dir: RomDirectory }) {
  const platforms = useLibraryStore((s) => s.platforms);
  const emulators = useLibraryStore((s) => s.emulators);
  const refresh = useLibraryStore((s) => s.refresh);
  const startScan = useScanStore((s) => s.start);
  const activeScanId = useScanStore((s) => s.activeScanId);

  async function update(patch: Partial<RomDirectory>) {
    await api.saveRomDirectory({ ...dir, ...patch });
    await refresh();
  }

  return (
    <div className="glass flex items-center gap-3 rounded-xl px-4 py-3">
      <Switch
        checked={dir.enabled}
        onCheckedChange={(v) => void update({ enabled: v })}
        aria-label={`Enable scanning of ${dir.path}`}
      />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm">{dir.path}</div>
        <div className="text-[11px] text-ink-faint">
          {(platforms.find((p) => p.id === dir.platformId)?.extensions ?? []).map((e) => `.${e}`).join(" ")}
        </div>
      </div>
      <Select value={dir.platformId} onValueChange={(v) => void update({ platformId: v })}>
        <SelectTrigger className="h-8 w-40 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {platforms
            .filter((p) => p.id !== "steam")
            .map((p) => (
              <SelectItem key={p.id} value={p.id}>
                {p.name}
              </SelectItem>
            ))}
        </SelectContent>
      </Select>
      <Select
        value={dir.emulatorId ?? "__default"}
        onValueChange={(v) => void update({ emulatorId: v === "__default" ? null : v })}
      >
        <SelectTrigger className="h-8 w-40 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="__default">Platform default</SelectItem>
          {emulators.map((e) => (
            <SelectItem key={e.id} value={e.id}>
              {e.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button
        variant="glass"
        size="iconSm"
        aria-label="Scan this directory"
        disabled={activeScanId !== null}
        onClick={() => startScan({ kind: "directory", directoryId: dir.id })}
      >
        <RefreshCw />
      </Button>
      <Button
        variant="ghost"
        size="iconSm"
        aria-label="Remove directory"
        onClick={async () => {
          await api.deleteRomDirectory(dir.id);
          await refresh();
        }}
      >
        <Trash2 />
      </Button>
    </div>
  );
}

function EmulatorsSection() {
  const emulators = useLibraryStore((s) => s.emulators);
  const platforms = useLibraryStore((s) => s.platforms);
  const refresh = useLibraryStore((s) => s.refresh);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [editing, setEditing] = useState<Emulator | null>(null);
  const [testResult, setTestResult] = useState<Record<string, string>>({});

  async function testEmulator(emulator: Emulator) {
    try {
      const result = await api.testEmulatorConfig(emulator);
      setTestResult((r) => ({ ...r, [emulator.id]: `✓ ${result}` }));
    } catch (e) {
      setTestResult((r) => ({ ...r, [emulator.id]: `✗ ${api.errorMessage(e)}` }));
    }
  }

  return (
    <div>
      <div className="flex items-start justify-between">
        <SectionTitle title="Emulators" description="Connected emulators and their launch configuration." />
        <Button
          onClick={() => {
            setEditing(null);
            setWizardOpen(true);
          }}
        >
          <Plus /> Connect emulator
        </Button>
      </div>

      {emulators.length === 0 && (
        <p className="text-sm text-ink-faint">
          No emulators yet. Use “Connect emulator” for a guided setup with RetroArch, Dolphin, PCSX2, Ryujinx
          and more.
        </p>
      )}

      <div className="space-y-3">
        {emulators.map((emulator) => (
          <div key={emulator.id} className="glass rounded-xl px-5 py-4">
            <div className="flex items-center gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-semibold">{emulator.name}</span>
                  <Badge variant={emulator.enabled ? "accent" : "default"}>
                    {emulator.emulatorType}
                  </Badge>
                </div>
                <div className="mt-0.5 truncate text-xs text-ink-faint">{emulator.executablePath}</div>
                <div className="mt-1 flex flex-wrap gap-1">
                  {emulator.platforms.map((id) => (
                    <Badge key={id}>{platforms.find((p) => p.id === id)?.shortName ?? id}</Badge>
                  ))}
                </div>
              </div>
              <Switch
                checked={emulator.enabled}
                aria-label={`Enable ${emulator.name}`}
                onCheckedChange={async (v) => {
                  await api.saveEmulator({ ...emulator, enabled: v });
                  await refresh();
                }}
              />
              <Button variant="glass" size="sm" onClick={() => testEmulator(emulator)}>
                Test
              </Button>
              <Button
                variant="glass"
                size="iconSm"
                aria-label={`Edit ${emulator.name}`}
                onClick={() => {
                  setEditing(emulator);
                  setWizardOpen(true);
                }}
              >
                <Pencil />
              </Button>
              <Button
                variant="ghost"
                size="iconSm"
                aria-label={`Delete ${emulator.name}`}
                onClick={async () => {
                  await api.deleteEmulator(emulator.id);
                  await refresh();
                }}
              >
                <Trash2 />
              </Button>
            </div>
            <div className="mt-2 select-text font-mono text-[11px] text-ink-faint">{emulator.commandTemplate}</div>
            {testResult[emulator.id] && (
              <div
                className={cn(
                  "mt-2 select-text break-all rounded-lg px-3 py-2 font-mono text-[11px]",
                  testResult[emulator.id].startsWith("✓")
                    ? "bg-success/10 text-success"
                    : "bg-danger/10 text-danger",
                )}
              >
                {testResult[emulator.id]}
              </div>
            )}
          </div>
        ))}
      </div>

      <EmulatorWizard open={wizardOpen} onOpenChange={setWizardOpen} editing={editing} />
    </div>
  );
}

function PlatformsSection() {
  const platforms = useLibraryStore((s) => s.platforms);
  const emulators = useLibraryStore((s) => s.emulators);
  const refresh = useLibraryStore((s) => s.refresh);

  return (
    <div>
      <SectionTitle title="Platforms" description="Default emulator and file extensions per platform." />
      <div className="space-y-1.5">
        {platforms.map((platform) => (
          <div key={platform.id} className="flex items-center gap-4 border-b border-edge py-3 last:border-b-0">
            <div className="min-w-0 flex-1">
              <div className="text-sm font-medium">{platform.name}</div>
              <div className="text-[11px] text-ink-faint">
                {platform.extensions.length > 0
                  ? platform.extensions.map((e) => `.${e}`).join("  ")
                  : platform.id === "steam"
                    ? "Managed by Steam"
                    : "No extensions"}
              </div>
            </div>
            {platform.id !== "steam" && (
              <Select
                value={platform.defaultEmulatorId ?? "__none"}
                onValueChange={async (v) => {
                  await api.setPlatformDefaultEmulator(platform.id, v === "__none" ? null : v);
                  await refresh();
                }}
              >
                <SelectTrigger className="h-9 w-52 text-xs">
                  <SelectValue placeholder="Default emulator" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none">No default emulator</SelectItem>
                  {emulators
                    .filter((e) => e.platforms.includes(platform.id) || e.platforms.length === 0)
                    .map((e) => (
                      <SelectItem key={e.id} value={e.id}>
                        {e.name}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function ProvidersSection() {
  const settings = useSettingsStore();
  const providers = useSettingsStore((s) => s.providers);
  const activeScanId = useScanStore((s) => s.activeScanId);
  const progress = useScanStore((s) => s.progress);
  // The kind of operation currently running, if any. Each button is disabled
  // only while *its own* kind runs — a plain library scan never blocks them.
  const activeSource = activeScanId !== null ? (progress?.source ?? "scan") : null;
  const downloadBusy = activeSource === "launchbox";
  const matchBusy = activeSource === "artwork" || activeSource === "launchbox";
  const [key, setKey] = useState("");
  const [enrichError, setEnrichError] = useState<string | null>(null);
  const [lbCount, setLbCount] = useState<number | null>(null);

  const refreshLbStatus = () =>
    void api.launchboxStatus().then((s) => setLbCount(s.count)).catch(() => setLbCount(0));

  useEffect(() => {
    setKey(settings.get("providers.steamgriddb.apiKey", ""));
    refreshLbStatus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.loaded]);

  // Refresh the indexed-games count when a download finishes.
  useEffect(() => {
    if (activeScanId === null) refreshLbStatus();
  }, [activeScanId]);

  async function fetchMissingArtwork() {
    setEnrichError(null);
    try {
      await api.enrichArtwork();
    } catch (e) {
      setEnrichError(api.errorMessage(e));
    }
  }

  async function downloadLaunchbox() {
    setEnrichError(null);
    try {
      await api.downloadLaunchboxDb();
    } catch (e) {
      setEnrichError(api.errorMessage(e));
    }
  }

  return (
    <div>
      <SectionTitle
        title="Metadata Providers"
        description="Sources for metadata and artwork. Moonlight fills ROM, Steam and Switch games automatically on scan; the options here add coverage. All metadata can also be edited by hand."
      />

      {/* LaunchBox Games Database — keyless retro metadata */}
      <div className="mb-4 rounded-xl glass p-5">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">LaunchBox Games Database</span>
              <Badge variant={lbCount && lbCount > 0 ? "success" : "default"}>
                {lbCount && lbCount > 0 ? `${lbCount.toLocaleString()} games` : "Not downloaded"}
              </Badge>
            </div>
            <p className="mt-1 text-xs text-ink-dim">
              Keyless retro metadata — descriptions, developer, publisher, genres and release dates for
              console games (NES, SNES, N64, GameCube, PS1/PS2, Genesis, Dreamcast and more). One-time
              ~100&nbsp;MB download, cached locally and matched automatically on scan.
            </p>
          </div>
          <Button className="shrink-0" disabled={downloadBusy} onClick={downloadLaunchbox}>
            <Database /> {lbCount && lbCount > 0 ? "Update database" : "Download database"}
          </Button>
        </div>
        <p className="mt-2 text-[10px] text-ink-faint">Metadata courtesy of the LaunchBox Games Database (gamesdb.launchbox-app.com).</p>
      </div>

      <div className="mb-4 rounded-xl glass p-5">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="text-sm font-medium">Match metadata & artwork now</div>
            <p className="mt-1 text-xs text-ink-dim">
              Runs automatically after every scan. Fills release dates, developers, genres and box art
              for ROM, Steam and Switch games from the keyless sources above — no API key required.
            </p>
          </div>
          <Button className="shrink-0" disabled={matchBusy} onClick={fetchMissingArtwork}>
            <Image /> Match now
          </Button>
        </div>
        {enrichError && (
          <div className="mt-3 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-xs text-danger">
            {enrichError}
          </div>
        )}
      </div>

      {providers.map((provider) => (
        <div key={provider.id} className="rounded-xl glass p-5">
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center gap-2">
                <span className="font-semibold">{provider.name}</span>
                <Badge variant={provider.configured ? "success" : "default"}>
                  {provider.configured ? "Configured" : "Not configured"}
                </Badge>
              </div>
              <p className="mt-1 text-xs text-ink-dim">{provider.attribution}</p>
            </div>
          </div>
          {provider.requiresApiKey && (
            <div className="mt-4 space-y-1.5">
              <Label htmlFor={`key-${provider.id}`}>API key</Label>
              <div className="flex gap-2">
                <Input
                  id={`key-${provider.id}`}
                  type="password"
                  value={key}
                  onChange={(e) => setKey(e.target.value)}
                  placeholder="Paste your SteamGridDB API key"
                  className="flex-1"
                />
                <Button
                  variant="glass"
                  onClick={() => void settings.set(`providers.${provider.id}.apiKey`, key)}
                >
                  Save
                </Button>
              </div>
              <p className="text-xs text-ink-faint">
                Get a free key at steamgriddb.com → Preferences → API. Stored locally only.
              </p>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

function ArtworkSection() {
  const appInfo = useSettingsStore((s) => s.appInfo);
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  useEffect(() => {
    void api.getDiagnostics().then(setDiag);
  }, []);
  return (
    <div>
      <SectionTitle title="Artwork Storage" description="Where downloaded and imported images live." />
      <SettingRow label="Artwork directory" description={appInfo?.artworkDir ?? "…"}>
        <span className="text-sm text-ink-dim">{diag ? formatBytes(diag.artworkSizeBytes) : "—"}</span>
      </SettingRow>
      <SettingRow
        label="Artwork records"
        description="Images tracked in the library database, including provider alternatives."
      >
        <span className="text-sm text-ink-dim">{diag?.counts["artwork"] ?? "—"}</span>
      </SettingRow>
      <p className="mt-4 text-xs text-ink-faint">
        All artwork is copied into the application data directory so your library stays intact even if
        original files move. Deleting a game removes its artwork records.
      </p>
    </div>
  );
}

function AppearanceSection() {
  const settings = useSettingsStore();
  return (
    <div>
      <SectionTitle title="Appearance" description="Visual preferences." />
      <SettingRow label="Reduce motion" description="Minimize animations and transitions.">
        <Switch
          checked={settings.get("appearance.reduceMotion", false)}
          onCheckedChange={(v) => void settings.set("appearance.reduceMotion", v)}
          aria-label="Reduce motion"
        />
      </SettingRow>
      <SettingRow label="Ambient backdrop" description="Blurred artwork behind the interface for the selected game.">
        <Switch
          checked={settings.get("appearance.ambientBackdrop", true)}
          onCheckedChange={(v) => void settings.set("appearance.ambientBackdrop", v)}
          aria-label="Ambient backdrop"
        />
      </SettingRow>
      <SettingRow label="Interface scale" description="Scale the whole interface for couch or desktop use.">
        <Select
          value={String(settings.get("appearance.uiScale", 1))}
          onValueChange={(v) => void settings.set("appearance.uiScale", Number(v))}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="0.9">90%</SelectItem>
            <SelectItem value="1">100%</SelectItem>
            <SelectItem value="1.1">110%</SelectItem>
            <SelectItem value="1.25">125% (TV)</SelectItem>
            <SelectItem value="1.5">150% (TV)</SelectItem>
          </SelectContent>
        </Select>
      </SettingRow>
    </div>
  );
}

function InputSection() {
  return (
    <div>
      <SectionTitle title="Input" description="Keyboard and controller bindings." />
      <div className="space-y-2 text-sm">
        {[
          ["Arrow keys / D-pad / Left stick", "Move selection"],
          ["Enter / A", "Open game details"],
          ["Escape / B", "Close panel, go back"],
          ["F / Y", "Toggle favorite"],
          ["L / Start", "Launch focused game"],
          ["/ or Ctrl+F", "Focus search"],
        ].map(([keys, action]) => (
          <div key={keys} className="flex items-center justify-between border-b border-edge py-2.5 last:border-b-0">
            <span className="text-ink-dim">{action}</span>
            <kbd className="rounded-lg bg-glass-strong px-3 py-1 font-mono text-xs">{keys}</kbd>
          </div>
        ))}
      </div>
      <p className="mt-4 text-xs text-ink-faint">
        Controllers are detected automatically via the system gamepad API — connect one and start navigating.
      </p>
    </div>
  );
}

function ImportExportSection() {
  const [status, setStatus] = useState<string | null>(null);

  async function backup() {
    const dest = await save({ title: "Backup database", defaultPath: "moonlight-backup.db" });
    if (!dest) return;
    try {
      await api.backupDatabase(dest);
      setStatus(`Database backed up to ${dest}`);
    } catch (e) {
      setStatus(api.errorMessage(e));
    }
  }

  async function exportJson() {
    const dest = await save({ title: "Export library", defaultPath: "moonlight-library.json" });
    if (!dest) return;
    try {
      await api.exportLibrary(dest);
      setStatus(`Library exported to ${dest}`);
    } catch (e) {
      setStatus(api.errorMessage(e));
    }
  }

  return (
    <div>
      <SectionTitle title="Import & Export" description="Back up and export your library." />
      <SettingRow label="Back up database" description="Copy the SQLite database to a file of your choosing.">
        <Button variant="glass" onClick={backup}>
          <Database /> Back up…
        </Button>
      </SettingRow>
      <SettingRow label="Export library as JSON" description="Games, platforms, emulators, collections and settings.">
        <Button variant="glass" onClick={exportJson}>
          <FolderOpen /> Export…
        </Button>
      </SettingRow>
      {status && <p className="mt-4 select-text text-sm text-ink-dim">{status}</p>}
    </div>
  );
}

function DiagnosticsSection() {
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [logs, setLogs] = useState<string[] | null>(null);
  const [loadingLogs, setLoadingLogs] = useState(false);

  useEffect(() => {
    void api.getDiagnostics().then(setDiag);
  }, []);

  async function loadLogs() {
    setLoadingLogs(true);
    try {
      setLogs(await api.readLogs(300));
    } finally {
      setLoadingLogs(false);
    }
  }

  if (!diag) return <Spinner />;

  return (
    <div>
      <SectionTitle title="Diagnostics" description="Application health and logs." />
      <div className="grid grid-cols-2 gap-x-8">
        <SettingRow label="Version">
          <span className="text-sm text-ink-dim">{diag.version}</span>
        </SettingRow>
        <SettingRow label="Database size">
          <span className="text-sm text-ink-dim">{formatBytes(diag.dbSizeBytes)}</span>
        </SettingRow>
        {Object.entries(diag.counts).map(([table, count]) => (
          <SettingRow key={table} label={table.replace("_", " ")}>
            <span className="text-sm tabular-nums text-ink-dim">{count}</span>
          </SettingRow>
        ))}
      </div>
      <div className="mt-5 space-y-1 text-xs text-ink-faint">
        <p className="select-text">Data: {diag.dataDir}</p>
        <p className="select-text">Database: {diag.dbPath}</p>
        <p className="select-text">Logs: {diag.logDir}</p>
      </div>
      <div className="mt-6">
        <Button variant="glass" onClick={loadLogs} disabled={loadingLogs}>
          {loadingLogs ? <Spinner className="size-4" /> : <Info />} View recent logs
        </Button>
        {logs && (
          <pre className="mt-3 max-h-80 select-text overflow-auto rounded-xl bg-black/40 p-4 font-mono text-[11px] leading-relaxed text-ink-dim">
            {logs.length > 0 ? logs.join("\n") : "Log file is empty."}
          </pre>
        )}
      </div>
    </div>
  );
}
