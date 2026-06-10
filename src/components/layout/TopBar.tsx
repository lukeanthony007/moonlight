import { useEffect, useRef } from "react";
import { Filter, LayoutGrid, List, Plus, RefreshCw, Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Checkbox, Label, Tooltip } from "@/components/ui/misc";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { SORT_LABELS, type SortMode } from "@/lib/sort";
import { useUiStore } from "@/stores/uiStore";
import { useScanStore } from "@/stores/scanStore";
import { useLibraryStore } from "@/stores/libraryStore";
import { allGenres } from "@/lib/sort";
import { cn } from "@/lib/utils";

interface TopBarProps {
  title: string;
  onAddGame: () => void;
}

export function TopBar({ title, onAddGame }: TopBarProps) {
  const search = useUiStore((s) => s.search);
  const setSearch = useUiStore((s) => s.setSearch);
  const sortMode = useUiStore((s) => s.sortMode);
  const setSortMode = useUiStore((s) => s.setSortMode);
  const viewMode = useUiStore((s) => s.viewMode);
  const setViewMode = useUiStore((s) => s.setViewMode);
  const installedOnly = useUiStore((s) => s.installedOnly);
  const setInstalledOnly = useUiStore((s) => s.setInstalledOnly);
  const showHidden = useUiStore((s) => s.showHidden);
  const setShowHidden = useUiStore((s) => s.setShowHidden);
  const genre = useUiStore((s) => s.genre);
  const setGenre = useUiStore((s) => s.setGenre);
  const sourceType = useUiStore((s) => s.sourceType);
  const setSourceType = useUiStore((s) => s.setSourceType);

  const games = useLibraryStore((s) => s.games);
  const activeScanId = useScanStore((s) => s.activeScanId);
  const startScan = useScanStore((s) => s.start);

  const searchRef = useRef<HTMLInputElement>(null);
  const genres = allGenres(games);
  const filtersActive = installedOnly || showHidden || genre !== null || sourceType !== null;

  // Ctrl/Cmd+F or "/" focuses search.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const typing = (e.target as HTMLElement).closest("input, textarea");
      if (((e.ctrlKey || e.metaKey) && e.key === "f") || (e.key === "/" && !typing)) {
        e.preventDefault();
        searchRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <header className="flex items-center gap-3 px-8 pb-5 pt-7">
      <h1 className="min-w-0 flex-1 truncate text-2xl font-bold tracking-tight">{title}</h1>

      <div className="relative w-72">
        <Search className="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-ink-faint" />
        <Input
          ref={searchRef}
          aria-label="Search games"
          placeholder="Search games…"
          className="pl-10 pr-9"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setSearch("");
              (e.target as HTMLInputElement).blur();
            }
          }}
        />
        {search && (
          <button
            aria-label="Clear search"
            className="absolute right-3 top-1/2 -translate-y-1/2 text-ink-faint hover:text-ink cursor-pointer"
            onClick={() => setSearch("")}
          >
            <X className="size-4" />
          </button>
        )}
      </div>

      <Select value={sortMode} onValueChange={(v) => setSortMode(v as SortMode)}>
        <SelectTrigger className="w-52" aria-label="Sort order">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {Object.entries(SORT_LABELS).map(([value, label]) => (
            <SelectItem key={value} value={value}>
              {label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button variant="glass" size="icon" aria-label="Filters" className={cn(filtersActive && "text-accent")}>
            <Filter />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            align="end"
            sideOffset={8}
            className="glass-strong card-shadow z-50 w-64 rounded-xl bg-[#16161f]/95 p-4"
          >
            <div className="space-y-4">
              <label className="flex items-center justify-between gap-3 text-sm">
                Installed only
                <Checkbox checked={installedOnly} onCheckedChange={(v) => setInstalledOnly(v === true)} />
              </label>
              <label className="flex items-center justify-between gap-3 text-sm">
                Show hidden games
                <Checkbox checked={showHidden} onCheckedChange={(v) => setShowHidden(v === true)} />
              </label>
              <div className="space-y-1.5">
                <Label>Genre</Label>
                <Select value={genre ?? "__all"} onValueChange={(v) => setGenre(v === "__all" ? null : v)}>
                  <SelectTrigger className="h-9">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__all">All genres</SelectItem>
                    {genres.map((g) => (
                      <SelectItem key={g} value={g}>
                        {g}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label>Source</Label>
                <Select value={sourceType ?? "__all"} onValueChange={(v) => setSourceType(v === "__all" ? null : v)}>
                  <SelectTrigger className="h-9">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__all">All sources</SelectItem>
                    <SelectItem value="steam">Steam</SelectItem>
                    <SelectItem value="rom">ROM scan</SelectItem>
                    <SelectItem value="manual">Manually added</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <div className="glass flex rounded-xl p-1">
        <Tooltip content="Grid view">
          <button
            aria-label="Grid view"
            aria-pressed={viewMode === "grid"}
            className={cn(
              "focusable rounded-lg p-1.5 transition-colors cursor-pointer",
              viewMode === "grid" ? "bg-glass-strong text-ink" : "text-ink-faint hover:text-ink-dim",
            )}
            onClick={() => setViewMode("grid")}
          >
            <LayoutGrid className="size-4" />
          </button>
        </Tooltip>
        <Tooltip content="List view">
          <button
            aria-label="List view"
            aria-pressed={viewMode === "list"}
            className={cn(
              "focusable rounded-lg p-1.5 transition-colors cursor-pointer",
              viewMode === "list" ? "bg-glass-strong text-ink" : "text-ink-faint hover:text-ink-dim",
            )}
            onClick={() => setViewMode("list")}
          >
            <List className="size-4" />
          </button>
        </Tooltip>
      </div>

      <Tooltip content="Scan all sources">
        <Button
          variant="glass"
          size="icon"
          aria-label="Scan library"
          disabled={activeScanId !== null}
          onClick={() => startScan({ kind: "all" })}
        >
          <RefreshCw className={cn(activeScanId && "animate-spin")} />
        </Button>
      </Tooltip>

      <Tooltip content="Add game manually">
        <Button variant="glass" size="icon" aria-label="Add game" onClick={onAddGame}>
          <Plus />
        </Button>
      </Tooltip>
    </header>
  );
}
