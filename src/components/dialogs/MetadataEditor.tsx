import { useEffect, useMemo, useState } from "react";
import { Lock, LockOpen, RotateCcw, Search, Star } from "lucide-react";
import * as api from "@/lib/api";
import type { LibraryEntry, ProviderMatch } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input, Textarea } from "@/components/ui/input";
import { Badge, Label, Spinner, Tooltip } from "@/components/ui/misc";
import { useLibraryStore } from "@/stores/libraryStore";
import { useSettingsStore } from "@/stores/settingsStore";

interface MetadataEditorProps {
  game: LibraryEntry;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const LOCKABLE: { field: string; label: string }[] = [
  { field: "title", label: "Title" },
  { field: "description", label: "Description" },
  { field: "releaseDate", label: "Release date" },
  { field: "developer", label: "Developer" },
  { field: "publisher", label: "Publisher" },
  { field: "genres", label: "Genres" },
];

/** Full metadata editor with field locking and provider search. */
export function MetadataEditor({ game, open, onOpenChange }: MetadataEditorProps) {
  const refreshGame = useLibraryStore((s) => s.refreshGame);
  const providers = useSettingsStore((s) => s.providers);
  const configuredProvider = providers.find((p) => p.configured);

  const [form, setForm] = useState(() => toForm(game));
  const [locked, setLocked] = useState<string[]>(game.lockedFields);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Provider matching
  const [searching, setSearching] = useState(false);
  const [matches, setMatches] = useState<ProviderMatch[] | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setForm(toForm(game));
      setLocked(game.lockedFields);
      setMatches(null);
      setError(null);
    }
    // Reset when a different game is opened.
  }, [open, game]);

  const canRestore = useMemo(() => game.providerMetadata !== null, [game]);

  function toggleLock(field: string) {
    setLocked((prev) => (prev.includes(field) ? prev.filter((f) => f !== field) : [...prev, field]));
  }

  function lockButton(field: string) {
    const isLocked = locked.includes(field);
    return (
      <Tooltip content={isLocked ? "Unlock: allow automatic updates" : "Lock: protect from automatic updates"}>
        <button
          type="button"
          aria-label={`${isLocked ? "Unlock" : "Lock"} ${field}`}
          className={cn(
            "focusable rounded-md p-1 transition-colors cursor-pointer",
            isLocked ? "text-accent" : "text-ink-faint hover:text-ink-dim",
          )}
          onClick={() => toggleLock(field)}
        >
          {isLocked ? <Lock className="size-3.5" /> : <LockOpen className="size-3.5" />}
        </button>
      </Tooltip>
    );
  }

  async function save() {
    setBusy(true);
    setError(null);
    try {
      await api.updateGame(game.id, {
        title: form.title,
        description: form.description || null,
        releaseDate: form.releaseDate || null,
        developer: form.developer || null,
        publisher: form.publisher || null,
        genres: form.genres
          .split(",")
          .map((g) => g.trim())
          .filter(Boolean),
        region: form.region || null,
        series: form.series || null,
        userRating: form.userRating,
        lockedFields: locked,
      });
      await refreshGame(game.id);
      onOpenChange(false);
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function searchProvider() {
    if (!configuredProvider) return;
    setSearching(true);
    setError(null);
    try {
      const year = form.releaseDate ? Number(form.releaseDate.slice(0, 4)) || null : null;
      const results = await api.searchMetadata(configuredProvider.id, {
        title: form.title,
        platformId: game.platformId,
        region: game.region,
        releaseYear: year,
      });
      setMatches(results.slice(0, 6));
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setSearching(false);
    }
  }

  async function applyMatch(match: ProviderMatch) {
    setApplyingId(match.providerGameId);
    setError(null);
    try {
      await api.applyProviderMatch(game.id, match.provider, match.providerGameId, true);
      await refreshGame(game.id);
      onOpenChange(false);
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setApplyingId(null);
    }
  }

  async function restore() {
    setBusy(true);
    try {
      await api.restoreProviderMetadata(game.id);
      await refreshGame(game.id);
      onOpenChange(false);
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Edit metadata</DialogTitle>
          <DialogDescription>
            Edit fields manually, or fetch from a provider. Locked fields are never overwritten by automatic
            updates.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-2 gap-4">
          <div className="col-span-2 space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="meta-title">Title</Label>
              {lockButton("title")}
            </div>
            <Input id="meta-title" value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} />
          </div>

          <div className="col-span-2 space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="meta-desc">Description</Label>
              {lockButton("description")}
            </div>
            <Textarea
              id="meta-desc"
              value={form.description}
              onChange={(e) => setForm({ ...form, description: e.target.value })}
              rows={4}
            />
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="meta-release">Release date</Label>
              {lockButton("releaseDate")}
            </div>
            <Input
              id="meta-release"
              type="date"
              value={form.releaseDate}
              onChange={(e) => setForm({ ...form, releaseDate: e.target.value })}
            />
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="meta-genres">Genres (comma separated)</Label>
              {lockButton("genres")}
            </div>
            <Input
              id="meta-genres"
              value={form.genres}
              onChange={(e) => setForm({ ...form, genres: e.target.value })}
              placeholder="Action, Platformer"
            />
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="meta-dev">Developer</Label>
              {lockButton("developer")}
            </div>
            <Input id="meta-dev" value={form.developer} onChange={(e) => setForm({ ...form, developer: e.target.value })} />
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="meta-pub">Publisher</Label>
              {lockButton("publisher")}
            </div>
            <Input id="meta-pub" value={form.publisher} onChange={(e) => setForm({ ...form, publisher: e.target.value })} />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="meta-region">Region</Label>
            <Input id="meta-region" value={form.region} onChange={(e) => setForm({ ...form, region: e.target.value })} />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="meta-series">Series</Label>
            <Input id="meta-series" value={form.series} onChange={(e) => setForm({ ...form, series: e.target.value })} />
          </div>

          <div className="col-span-2 space-y-1.5">
            <Label>Your rating</Label>
            <div className="flex items-center gap-1">
              {[1, 2, 3, 4, 5].map((n) => (
                <button
                  key={n}
                  type="button"
                  aria-label={`Rate ${n} of 5`}
                  className="focusable rounded p-0.5 cursor-pointer"
                  onClick={() => setForm({ ...form, userRating: form.userRating === n ? null : n })}
                >
                  <Star
                    className={cn(
                      "size-5 transition-colors",
                      form.userRating !== null && n <= form.userRating
                        ? "fill-accent text-accent"
                        : "text-ink-faint hover:text-ink-dim",
                    )}
                  />
                </button>
              ))}
              {form.userRating !== null && (
                <button
                  type="button"
                  className="ml-2 text-xs text-ink-faint hover:text-ink-dim cursor-pointer"
                  onClick={() => setForm({ ...form, userRating: null })}
                >
                  Clear
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Provider matching */}
        <div className="rounded-xl glass p-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-sm font-medium">Fetch from provider</div>
              <div className="text-xs text-ink-dim">
                {configuredProvider
                  ? `Search ${configuredProvider.name} for this game and pull metadata + artwork.`
                  : "No provider configured. Add an API key in Settings → Metadata Providers."}
              </div>
            </div>
            <div className="flex gap-2">
              {canRestore && (
                <Tooltip content="Restore the last provider metadata over manual edits">
                  <Button variant="outline" size="sm" onClick={restore} disabled={busy}>
                    <RotateCcw /> Restore
                  </Button>
                </Tooltip>
              )}
              <Button variant="glass" size="sm" onClick={searchProvider} disabled={!configuredProvider || searching}>
                {searching ? <Spinner className="size-4" /> : <Search />} Search
              </Button>
            </div>
          </div>

          {matches && (
            <div className="mt-3 space-y-1.5">
              {matches.length === 0 && <p className="text-xs text-ink-faint">No matches found.</p>}
              {matches.map((match) => (
                <div
                  key={match.providerGameId}
                  className="flex items-center justify-between gap-3 rounded-lg bg-glass px-3 py-2"
                >
                  <div className="min-w-0">
                    <span className="text-sm">{match.title}</span>
                    {match.releaseYear && <span className="ml-2 text-xs text-ink-faint">{match.releaseYear}</span>}
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    <Badge
                      variant={match.confidence > 0.7 ? "success" : match.confidence > 0.4 ? "accent" : "default"}
                    >
                      {Math.round(match.confidence * 100)}% match
                    </Badge>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={applyingId !== null}
                      onClick={() => applyMatch(match)}
                    >
                      {applyingId === match.providerGameId ? <Spinner className="size-3.5" /> : "Apply"}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {error && (
          <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
        )}

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={save} disabled={busy || !form.title.trim()}>
            {busy ? "Saving…" : "Save changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function toForm(game: LibraryEntry) {
  return {
    title: game.title,
    description: game.description ?? "",
    releaseDate: game.releaseDate ?? "",
    developer: game.developer ?? "",
    publisher: game.publisher ?? "",
    genres: game.genres.join(", "),
    region: game.region ?? "",
    series: game.series ?? "",
    userRating: game.userRating,
  };
}

// Re-export so the lock list stays in one place if other UIs need it.
export { LOCKABLE };
