import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, ArrowRight, CheckCircle2, FolderOpen, Play, Sparkles } from "lucide-react";
import * as api from "@/lib/api";
import type { DetectedCore, Emulator, EmulatorPreset, RomDirectory } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Checkbox, Label, Spinner } from "@/components/ui/misc";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLibraryStore } from "@/stores/libraryStore";
import { useScanStore } from "@/stores/scanStore";
import { useSettingsStore } from "@/stores/settingsStore";

interface EmulatorWizardProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Existing emulator to edit, or null for the guided "connect" flow. */
  editing?: Emulator | null;
}

type Step = "type" | "executable" | "platforms" | "directories" | "template" | "test" | "done";
const STEP_ORDER: Step[] = ["type", "executable", "platforms", "directories", "template", "test", "done"];

interface DirDraft {
  path: string;
  platformId: string;
}

/** Guided flow: pick emulator type → executable → platforms → ROM dirs →
 * template → test → scan. */
export function EmulatorWizard({ open: isOpen, onOpenChange, editing }: EmulatorWizardProps) {
  const platforms = useLibraryStore((s) => s.platforms);
  const refresh = useLibraryStore((s) => s.refresh);
  const startScan = useScanStore((s) => s.start);
  const settings = useSettingsStore();

  const [presets, setPresets] = useState<EmulatorPreset[]>([]);
  const [step, setStep] = useState<Step>("type");
  const [preset, setPreset] = useState<EmulatorPreset | null>(null);
  const [name, setName] = useState("");
  const [executable, setExecutable] = useState("");
  const [selectedPlatforms, setSelectedPlatforms] = useState<string[]>([]);
  const [template, setTemplate] = useState("{executable} {gamePath}");
  const [dirs, setDirs] = useState<DirDraft[]>([]);
  const [cores, setCores] = useState<DetectedCore[]>([]);
  const [coreMap, setCoreMap] = useState<Record<string, string>>({});
  const [detecting, setDetecting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [savedEmulator, setSavedEmulator] = useState<Emulator | null>(null);

  const isRetroArch = preset?.id === "retroarch" || editing?.emulatorType === "retroarch";

  useEffect(() => {
    if (!isOpen) return;
    void api.getEmulatorPresets().then(setPresets);
    setError(null);
    setTestResult(null);
    setSavedEmulator(null);
    if (editing) {
      setName(editing.name);
      setExecutable(editing.executablePath);
      setSelectedPlatforms(editing.platforms);
      setTemplate(editing.commandTemplate);
      setStep("executable");
      setPreset(null);
    } else {
      setStep("type");
      setPreset(null);
      setName("");
      setExecutable("");
      setSelectedPlatforms([]);
      setTemplate("{executable} {gamePath}");
      setDirs([]);
    }
  }, [isOpen, editing]);

  const stepIndex = STEP_ORDER.indexOf(step);

  function choosePreset(p: EmulatorPreset) {
    setPreset(p);
    setName(p.name === "Custom emulator" ? "" : p.name);
    setSelectedPlatforms(p.platforms);
    setTemplate(p.commandTemplate);
    setStep("executable");
    // Try auto-detecting the executable.
    setDetecting(true);
    api
      .detectEmulatorExecutable(p.id)
      .then((path) => {
        if (path) setExecutable(path);
      })
      .finally(() => setDetecting(false));
  }

  async function pickExecutable() {
    const selected = await open({ multiple: false, title: "Locate emulator executable" });
    if (typeof selected === "string") setExecutable(selected);
  }

  async function enterPlatforms() {
    setStep("platforms");
    if (isRetroArch) {
      const detected = await api.detectRetroarchCores();
      setCores(detected);
      // Pre-fill the core map from detection + existing settings.
      const existing = settings.get<Record<string, string>>("retroarch.coreMap", {});
      const map: Record<string, string> = { ...existing };
      for (const core of detected) {
        if (core.platformId && !map[core.platformId]) map[core.platformId] = core.path;
      }
      setCoreMap(map);
    }
  }

  async function addDirectory() {
    const selected = await open({ directory: true, multiple: false, title: "Select a game directory" });
    if (typeof selected === "string") {
      setDirs((d) => [...d, { path: selected, platformId: selectedPlatforms[0] ?? "windows" }]);
    }
  }

  function buildEmulator(): Emulator {
    return {
      id: editing?.id ?? savedEmulator?.id ?? "",
      name: name.trim() || preset?.name || "Emulator",
      executablePath: executable.trim(),
      emulatorType: editing?.emulatorType ?? preset?.emulatorType ?? "custom",
      platforms: selectedPlatforms,
      commandTemplate: template.trim(),
      coreName: editing?.coreName ?? null,
      workingDirectory: editing?.workingDirectory ?? null,
      environment: editing?.environment ?? {},
      enabled: true,
    };
  }

  async function runTest() {
    setBusy(true);
    setError(null);
    try {
      const commandLine = await api.testEmulatorConfig(buildEmulator());
      setTestResult({ ok: true, message: commandLine });
    } catch (e) {
      setTestResult({ ok: false, message: api.errorMessage(e) });
    } finally {
      setBusy(false);
    }
  }

  async function saveAll(scanAfter: boolean) {
    setBusy(true);
    setError(null);
    try {
      const emulator = await api.saveEmulator(buildEmulator());
      setSavedEmulator(emulator);
      // Associate as default emulator for its platforms when unset.
      for (const platformId of emulator.platforms) {
        const platform = platforms.find((p) => p.id === platformId);
        if (platform && !platform.defaultEmulatorId) {
          await api.setPlatformDefaultEmulator(platformId, emulator.id);
        }
      }
      if (isRetroArch && Object.keys(coreMap).length > 0) {
        await settings.set("retroarch.coreMap", coreMap);
      }
      const savedDirs: RomDirectory[] = [];
      for (const dir of dirs) {
        savedDirs.push(
          await api.saveRomDirectory({
            id: "",
            path: dir.path,
            platformId: dir.platformId,
            emulatorId: emulator.id,
            enabled: true,
          }),
        );
      }
      await refresh();
      if (scanAfter && savedDirs.length > 0) {
        onOpenChange(false);
        await startScan({ kind: "all" });
      } else {
        setStep("done");
      }
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const extensionsForPlatform = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const p of platforms) map.set(p.id, p.extensions);
    return map;
  }, [platforms]);

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{editing ? `Edit ${editing.name}` : "Connect an emulator"}</DialogTitle>
          <DialogDescription>
            Step {Math.min(stepIndex + 1, 6)} of 6 —{" "}
            {
              {
                type: "Choose the emulator type",
                executable: "Locate the executable",
                platforms: "Select platforms" + (isRetroArch ? " and cores" : ""),
                directories: "Select game directories",
                template: "Review the launch command",
                test: "Test the configuration",
                done: "All set",
              }[step]
            }
          </DialogDescription>
        </DialogHeader>

        {/* Step content */}
        {step === "type" && (
          <div className="grid grid-cols-2 gap-3">
            {presets.map((p) => (
              <button
                key={p.id}
                className="focusable glass flex flex-col items-start gap-1 rounded-xl p-4 text-left transition-colors hover:border-edge-strong cursor-pointer"
                onClick={() => choosePreset(p)}
              >
                <span className="font-semibold">{p.name}</span>
                <span className="text-xs text-ink-dim">
                  {p.id === "custom"
                    ? "Any other emulator or launcher"
                    : p.platforms
                        .slice(0, 4)
                        .map((id) => platforms.find((pl) => pl.id === id)?.shortName ?? id)
                        .join(", ") + (p.platforms.length > 4 ? "…" : "")}
                </span>
              </button>
            ))}
          </div>
        )}

        {step === "executable" && (
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="wiz-name">Name</Label>
              <Input id="wiz-name" value={name} onChange={(e) => setName(e.target.value)} placeholder="Emulator name" />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="wiz-exe">Executable path</Label>
              <div className="flex gap-2">
                <Input
                  id="wiz-exe"
                  value={executable}
                  onChange={(e) => setExecutable(e.target.value)}
                  placeholder="/usr/bin/retroarch"
                  className="flex-1"
                />
                <Button variant="glass" size="icon" aria-label="Browse for executable" onClick={pickExecutable}>
                  <FolderOpen />
                </Button>
              </div>
              {detecting && (
                <p className="flex items-center gap-2 text-xs text-ink-dim">
                  <Spinner className="size-3.5" /> Looking for an installed copy…
                </p>
              )}
              {!detecting && executable && (
                <p className="flex items-center gap-1.5 text-xs text-success">
                  <Sparkles className="size-3.5" /> Executable set
                </p>
              )}
            </div>
          </div>
        )}

        {step === "platforms" && (
          <div className="max-h-96 space-y-1.5 overflow-y-auto pr-1">
            {platforms
              .filter((p) => p.id !== "steam")
              .map((platform) => {
                const checked = selectedPlatforms.includes(platform.id);
                return (
                  <div
                    key={platform.id}
                    className={cn(
                      "flex items-center justify-between gap-3 rounded-xl border px-4 py-2.5 transition-colors",
                      checked ? "glass border-edge-strong" : "border-transparent",
                    )}
                  >
                    <label className="flex flex-1 cursor-pointer items-center gap-3 text-sm">
                      <Checkbox
                        checked={checked}
                        onCheckedChange={(v) =>
                          setSelectedPlatforms((prev) =>
                            v === true ? [...prev, platform.id] : prev.filter((id) => id !== platform.id),
                          )
                        }
                      />
                      {platform.name}
                    </label>
                    {isRetroArch && checked && (
                      <Select
                        value={coreMap[platform.id] ?? "__none"}
                        onValueChange={(v) =>
                          setCoreMap((m) => {
                            const next = { ...m };
                            if (v === "__none") delete next[platform.id];
                            else next[platform.id] = v;
                            return next;
                          })
                        }
                      >
                        <SelectTrigger className="h-8 w-56 text-xs">
                          <SelectValue placeholder="Choose core…" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="__none">No core</SelectItem>
                          {cores.map((core) => (
                            <SelectItem key={core.path} value={core.path}>
                              {core.displayName}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    )}
                  </div>
                );
              })}
            {isRetroArch && cores.length === 0 && (
              <p className="px-1 pt-2 text-xs text-ink-dim">
                No libretro cores detected. Install cores via RetroArch's Online Updater, then re-run this step.
              </p>
            )}
          </div>
        )}

        {step === "directories" && (
          <div className="space-y-3">
            {dirs.length === 0 && (
              <p className="text-sm text-ink-dim">
                Add one or more folders containing games for this emulator. Each folder is scanned for the
                platform's file extensions.
              </p>
            )}
            {dirs.map((dir, index) => (
              <div key={index} className="glass flex items-center gap-3 rounded-xl px-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm">{dir.path}</div>
                  <div className="text-[11px] text-ink-faint">
                    {(extensionsForPlatform.get(dir.platformId) ?? []).map((e) => `.${e}`).join("  ") ||
                      "all files"}
                  </div>
                </div>
                <Select
                  value={dir.platformId}
                  onValueChange={(v) => setDirs((d) => d.map((x, i) => (i === index ? { ...x, platformId: v } : x)))}
                >
                  <SelectTrigger className="h-8 w-44 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {(selectedPlatforms.length ? selectedPlatforms : platforms.map((p) => p.id)).map((id) => (
                      <SelectItem key={id} value={id}>
                        {platforms.find((p) => p.id === id)?.name ?? id}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setDirs((d) => d.filter((_, i) => i !== index))}
                >
                  Remove
                </Button>
              </div>
            ))}
            <Button variant="glass" onClick={addDirectory}>
              <FolderOpen /> Add directory…
            </Button>
          </div>
        )}

        {step === "template" && (
          <div className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="wiz-template">Launch command template</Label>
              <Input
                id="wiz-template"
                value={template}
                onChange={(e) => setTemplate(e.target.value)}
                className="font-mono text-xs"
              />
            </div>
            <div className="rounded-xl glass px-4 py-3 text-xs leading-relaxed text-ink-dim">
              <p className="mb-1 font-medium text-ink">Placeholders</p>
              <p>
                <code className="text-accent">{"{executable}"}</code> — emulator binary ·{" "}
                <code className="text-accent">{"{gamePath}"}</code> — game file ·{" "}
                <code className="text-accent">{"{corePath}"}</code> — libretro core ·{" "}
                <code className="text-accent">{"{args}"}</code> — per-game extra arguments
              </p>
              <p className="mt-2">
                Arguments are passed directly to the process — never through a shell — so paths with spaces
                are safe.
              </p>
            </div>
          </div>
        )}

        {step === "test" && (
          <div className="space-y-4">
            <p className="text-sm text-ink-dim">
              Validates the executable and resolves the command template against a sample game.
            </p>
            <Button variant="glass" onClick={runTest} disabled={busy}>
              {busy ? <Spinner className="size-4" /> : <Play />} Run test
            </Button>
            {testResult && (
              <div
                className={cn(
                  "select-text rounded-xl border px-4 py-3 font-mono text-xs leading-relaxed break-all",
                  testResult.ok
                    ? "border-success/30 bg-success/10 text-success"
                    : "border-danger/30 bg-danger/10 text-danger",
                )}
              >
                {testResult.message}
              </div>
            )}
          </div>
        )}

        {step === "done" && (
          <div className="flex flex-col items-center gap-3 py-6 text-center">
            <CheckCircle2 className="size-12 text-success" />
            <p className="font-medium">Emulator saved</p>
            <p className="text-sm text-ink-dim">
              You can rescan anytime from the refresh button in the top bar.
            </p>
          </div>
        )}

        {error && (
          <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
        )}

        {/* Footer nav */}
        <div className="flex items-center justify-between">
          <Button
            variant="ghost"
            onClick={() => setStep(STEP_ORDER[Math.max(0, stepIndex - 1)])}
            disabled={stepIndex === 0 || step === "done"}
            className={cn(step === "done" && "invisible")}
          >
            <ArrowLeft /> Back
          </Button>
          <div className="flex gap-2">
            {step === "executable" && (
              <Button onClick={enterPlatforms} disabled={!executable.trim() || !name.trim()}>
                Next <ArrowRight />
              </Button>
            )}
            {step === "platforms" && (
              <Button onClick={() => setStep("directories")} disabled={selectedPlatforms.length === 0}>
                Next <ArrowRight />
              </Button>
            )}
            {step === "directories" && (
              <Button onClick={() => setStep("template")}>
                Next <ArrowRight />
              </Button>
            )}
            {step === "template" && (
              <Button onClick={() => setStep("test")} disabled={!template.includes("{executable}")}>
                Next <ArrowRight />
              </Button>
            )}
            {step === "test" && (
              <>
                <Button variant="outline" onClick={() => saveAll(false)} disabled={busy}>
                  Save
                </Button>
                <Button onClick={() => saveAll(true)} disabled={busy || dirs.length === 0}>
                  Save & scan now
                </Button>
              </>
            )}
            {step === "done" && <Button onClick={() => onOpenChange(false)}>Close</Button>}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
