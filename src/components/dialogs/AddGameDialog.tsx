import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import * as api from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/misc";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLibraryStore } from "@/stores/libraryStore";

interface AddGameDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: (gameId: string) => void;
}

/** Manually add a native game, ROM, script or shortcut. */
export function AddGameDialog({ open: isOpen, onOpenChange, onAdded }: AddGameDialogProps) {
  const platforms = useLibraryStore((s) => s.platforms);
  const emulators = useLibraryStore((s) => s.emulators);
  const refresh = useLibraryStore((s) => s.refresh);

  const [title, setTitle] = useState("");
  const [platformId, setPlatformId] = useState("windows");
  const [path, setPath] = useState("");
  const [emulatorId, setEmulatorId] = useState<string>("__none");
  const [launchArgs, setLaunchArgs] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pickFile() {
    const selected = await open({ multiple: false, title: "Select game file or executable" });
    if (typeof selected === "string") {
      setPath(selected);
      if (!title) {
        const base = selected.split(/[/\\]/).pop() ?? "";
        setTitle(base.replace(/\.[^.]+$/, "").replace(/_/g, " "));
      }
    }
  }

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      const entry = await api.addManualGame({
        title,
        platformId,
        path: path || null,
        emulatorId: emulatorId === "__none" ? null : emulatorId,
        launchArgs: launchArgs || null,
      });
      await refresh();
      onOpenChange(false);
      setTitle("");
      setPath("");
      setLaunchArgs("");
      onAdded(entry.id);
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add a game</DialogTitle>
          <DialogDescription>
            Add a native game, custom executable, ROM, script or shortcut to your library.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="add-title">Title</Label>
            <Input id="add-title" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Game title" />
          </div>

          <div className="space-y-1.5">
            <Label>Platform</Label>
            <Select value={platformId} onValueChange={setPlatformId}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {platforms.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="add-path">File or executable</Label>
            <div className="flex gap-2">
              <Input
                id="add-path"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="/path/to/game"
                className="flex-1"
              />
              <Button variant="glass" size="icon" aria-label="Browse" onClick={pickFile}>
                <FolderOpen />
              </Button>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label>Emulator (optional)</Label>
            <Select value={emulatorId} onValueChange={setEmulatorId}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="__none">None — launch directly</SelectItem>
                {emulators.map((e) => (
                  <SelectItem key={e.id} value={e.id}>
                    {e.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="add-args">Launch arguments (optional)</Label>
            <Input
              id="add-args"
              value={launchArgs}
              onChange={(e) => setLaunchArgs(e.target.value)}
              placeholder="--fullscreen"
            />
          </div>

          {error && (
            <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={busy || !title.trim()}>
            {busy ? "Adding…" : "Add game"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
