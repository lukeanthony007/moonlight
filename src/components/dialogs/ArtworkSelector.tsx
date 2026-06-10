import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, Download, FolderOpen, Link2, Search, Trash2 } from "lucide-react";
import * as api from "@/lib/api";
import { artworkUrl } from "@/lib/api";
import type { Artwork, ArtworkCandidate, ArtworkKind, LibraryEntry, ProviderMatch } from "@/lib/types";
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
import { Badge, Spinner, Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/misc";
import { useLibraryStore } from "@/stores/libraryStore";
import { useSettingsStore } from "@/stores/settingsStore";

const KINDS: { kind: ArtworkKind; label: string }[] = [
  { kind: "boxart", label: "Box art" },
  { kind: "background", label: "Background" },
  { kind: "logo", label: "Logo" },
  { kind: "banner", label: "Banner" },
  { kind: "icon", label: "Icon" },
  { kind: "screenshot", label: "Screenshot" },
];

interface ArtworkSelectorProps {
  game: LibraryEntry;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Per-slot artwork management: local files, URLs, and provider candidates. */
export function ArtworkSelector({ game, open: isOpen, onOpenChange }: ArtworkSelectorProps) {
  const refreshGame = useLibraryStore((s) => s.refreshGame);
  const providers = useSettingsStore((s) => s.providers);
  const provider = providers.find((p) => p.configured);

  const [kind, setKind] = useState<ArtworkKind>("boxart");
  const [existing, setExisting] = useState<Artwork[]>([]);
  const [candidates, setCandidates] = useState<ArtworkCandidate[] | null>(null);
  const [matches, setMatches] = useState<ProviderMatch[] | null>(null);
  const [selectedMatch, setSelectedMatch] = useState<ProviderMatch | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const art = await api.listArtwork(game.id);
    setExisting(art);
    await refreshGame(game.id);
  }, [game.id, refreshGame]);

  useEffect(() => {
    if (isOpen) {
      setError(null);
      setCandidates(null);
      setMatches(null);
      setSelectedMatch(null);
      void api.listArtwork(game.id).then(setExisting);
    }
  }, [isOpen, game.id]);

  const slotArt = existing.filter((a) => a.kind === kind && a.localPath);

  async function pickLocalFile() {
    const selected = await open({
      multiple: false,
      title: "Choose an image",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (typeof selected !== "string") return;
    setBusy("local");
    setError(null);
    try {
      await api.importArtworkFile(game.id, kind, selected);
      await reload();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  async function importUrl() {
    if (!url.trim()) return;
    setBusy("url");
    setError(null);
    try {
      await api.downloadArtwork(game.id, kind, url.trim(), null, true);
      setUrl("");
      await reload();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  async function searchProvider() {
    if (!provider) return;
    setBusy("search");
    setError(null);
    try {
      const results = await api.searchMetadata(provider.id, {
        title: game.title,
        platformId: game.platformId,
        region: game.region,
        releaseYear: game.releaseDate ? Number(game.releaseDate.slice(0, 4)) || null : null,
      });
      setMatches(results.slice(0, 5));
      // Auto-select a confident match.
      if (results.length > 0 && results[0].confidence > 0.7) {
        setSelectedMatch(results[0]);
        await loadCandidates(results[0]);
      }
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  async function loadCandidates(match: ProviderMatch) {
    if (!provider) return;
    setBusy("candidates");
    setError(null);
    try {
      const list = await api.getArtworkCandidates(provider.id, match.providerGameId, kind);
      setCandidates(list);
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  // Reload candidates when slot changes while a match is selected.
  useEffect(() => {
    if (selectedMatch) void loadCandidates(selectedMatch);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind]);

  async function downloadCandidate(candidate: ArtworkCandidate) {
    setBusy(candidate.url);
    setError(null);
    try {
      await api.downloadArtwork(game.id, kind, candidate.url, candidate.provider, true);
      await reload();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>Artwork — {game.title}</DialogTitle>
          <DialogDescription>
            Choose box art, backgrounds, logos, banners and icons. Your selections are never overwritten by
            scans or providers.
          </DialogDescription>
        </DialogHeader>

        <Tabs value={kind} onValueChange={(v) => setKind(v as ArtworkKind)}>
          <TabsList className="flex-wrap h-auto">
            {KINDS.map((k) => (
              <TabsTrigger key={k.kind} value={k.kind}>
                {k.label}
              </TabsTrigger>
            ))}
          </TabsList>

          {KINDS.map((k) => (
            <TabsContent key={k.kind} value={k.kind} className="mt-4 space-y-5">
              {/* Current artwork for this slot */}
              <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-ink-faint">
                  In library
                </h3>
                {slotArt.length === 0 ? (
                  <p className="text-sm text-ink-faint">No {k.label.toLowerCase()} yet.</p>
                ) : (
                  <div className="grid grid-cols-4 gap-3">
                    {slotArt.map((art) => (
                      <figure key={art.id} className="group relative overflow-hidden rounded-xl glass">
                        <img
                          src={artworkUrl(art.localPath)}
                          alt=""
                          className="aspect-[3/4] w-full object-cover"
                          style={k.kind === "background" || k.kind === "banner" ? { aspectRatio: "16/9" } : undefined}
                        />
                        {art.userSelected && (
                          <Badge variant="accent" className="absolute left-2 top-2">
                            <Check className="size-3" /> Selected
                          </Badge>
                        )}
                        <div className="absolute inset-x-0 bottom-0 flex justify-between gap-1 bg-gradient-to-t from-black/85 to-transparent p-2 opacity-0 transition-opacity group-hover:opacity-100">
                          {!art.userSelected && (
                            <Button
                              size="sm"
                              variant="glass"
                              onClick={async () => {
                                await api.selectArtwork(art.id);
                                await reload();
                              }}
                            >
                              Use this
                            </Button>
                          )}
                          <Button
                            size="iconSm"
                            variant="danger"
                            aria-label="Delete artwork"
                            onClick={async () => {
                              await api.deleteArtwork(art.id);
                              await reload();
                            }}
                          >
                            <Trash2 />
                          </Button>
                        </div>
                        {art.provider && (
                          <figcaption className="px-2 py-1 text-[10px] text-ink-faint">via {art.provider}</figcaption>
                        )}
                      </figure>
                    ))}
                  </div>
                )}
              </section>

              {/* Import controls */}
              <section className="grid grid-cols-2 gap-3">
                <Button variant="glass" onClick={pickLocalFile} disabled={busy !== null}>
                  {busy === "local" ? <Spinner className="size-4" /> : <FolderOpen />} Choose local image…
                </Button>
                <div className="flex gap-2">
                  <Input
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="Paste image URL…"
                    aria-label="Image URL"
                    onKeyDown={(e) => e.key === "Enter" && importUrl()}
                  />
                  <Button variant="glass" size="icon" aria-label="Download URL" onClick={importUrl} disabled={busy !== null || !url.trim()}>
                    {busy === "url" ? <Spinner className="size-4" /> : <Link2 />}
                  </Button>
                </div>
              </section>

              {/* Provider candidates */}
              <section>
                <div className="mb-2 flex items-center justify-between">
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-faint">
                    {provider ? `From ${provider.name}` : "Provider artwork"}
                  </h3>
                  <Button variant="outline" size="sm" onClick={searchProvider} disabled={!provider || busy !== null}>
                    {busy === "search" ? <Spinner className="size-4" /> : <Search />} Find artwork
                  </Button>
                </div>

                {!provider && (
                  <p className="text-sm text-ink-faint">
                    Configure a provider in Settings → Metadata Providers to fetch artwork alternatives.
                  </p>
                )}

                {matches && matches.length > 1 && (
                  <div className="mb-3 flex flex-wrap gap-1.5">
                    {matches.map((m) => (
                      <button
                        key={m.providerGameId}
                        className={cn(
                          "focusable rounded-full border px-3 py-1 text-xs transition-colors cursor-pointer",
                          selectedMatch?.providerGameId === m.providerGameId
                            ? "border-accent/40 bg-accent-soft text-accent"
                            : "border-edge text-ink-dim hover:text-ink",
                        )}
                        onClick={() => {
                          setSelectedMatch(m);
                          void loadCandidates(m);
                        }}
                      >
                        {m.title}
                        {m.releaseYear ? ` (${m.releaseYear})` : ""}
                      </button>
                    ))}
                  </div>
                )}

                {busy === "candidates" && <Spinner />}
                {candidates && candidates.length === 0 && busy === null && (
                  <p className="text-sm text-ink-faint">No {k.label.toLowerCase()} candidates found.</p>
                )}
                {candidates && candidates.length > 0 && (
                  <div className="grid max-h-72 grid-cols-4 gap-3 overflow-y-auto pr-1">
                    {candidates.map((candidate) => (
                      <figure key={candidate.url} className="group relative overflow-hidden rounded-xl glass">
                        <img
                          src={candidate.thumbnailUrl ?? candidate.url}
                          alt=""
                          loading="lazy"
                          className="aspect-[3/4] w-full object-cover"
                          style={kind === "background" || kind === "banner" ? { aspectRatio: "16/9" } : undefined}
                        />
                        <div className="absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 transition-opacity group-hover:opacity-100">
                          <Button size="sm" onClick={() => downloadCandidate(candidate)} disabled={busy !== null}>
                            {busy === candidate.url ? <Spinner className="size-4" /> : <Download />} Use
                          </Button>
                        </div>
                        {candidate.author && (
                          <figcaption className="truncate px-2 py-1 text-[10px] text-ink-faint">
                            by {candidate.author}
                          </figcaption>
                        )}
                      </figure>
                    ))}
                  </div>
                )}
                {provider && (
                  <p className="mt-3 text-[10px] text-ink-faint">{provider.attribution}</p>
                )}
              </section>

              {error && (
                <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
                  {error}
                </div>
              )}
            </TabsContent>
          ))}
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
