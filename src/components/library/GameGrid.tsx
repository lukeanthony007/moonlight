import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { LibraryEntry, Platform } from "@/lib/types";
import { GameCard } from "./GameCard";
import { GameListRow } from "./GameListRow";
import { useUiStore } from "@/stores/uiStore";

interface GameGridProps {
  games: LibraryEntry[];
  platforms: Platform[];
  runningGameIds: Set<string>;
  viewMode: "grid" | "list";
  onOpen: (game: LibraryEntry) => void;
  onLaunch: (game: LibraryEntry) => void;
  onToggleFavorite: (game: LibraryEntry) => void;
}

const CARD_MIN_WIDTH = 168;
const CARD_GAP = 18;
const CARD_EXTRA_HEIGHT = 62; // title block under the 3/4 artwork
const LIST_ROW_HEIGHT = 64;

/** Virtualized, keyboard/controller-navigable game grid. */
export function GameGrid({
  games,
  platforms,
  runningGameIds,
  viewMode,
  onOpen,
  onLaunch,
  onToggleFavorite,
}: GameGridProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const focusIndex = useUiStore((s) => s.focusIndex);
  const setFocusIndex = useUiStore((s) => s.setFocusIndex);
  const platformById = useMemo(() => new Map(platforms.map((p) => [p.id, p])), [platforms]);

  // Track measured width of the scroll container for column math.
  const [width, setWidth] = useState(1200);
  useEffect(() => {
    const el = parentRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const next = entries[0]?.contentRect.width;
      if (next) setWidth(next);
    });
    observer.observe(el);
    setWidth(el.clientWidth || 1200);
    return () => observer.disconnect();
  }, []);
  const columns =
    viewMode === "list" ? 1 : Math.max(2, Math.floor((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)));
  const cardWidth = viewMode === "list" ? width : (width - CARD_GAP * (columns - 1)) / columns;
  const rowHeight =
    viewMode === "list" ? LIST_ROW_HEIGHT : (cardWidth * 4) / 3 + CARD_EXTRA_HEIGHT + CARD_GAP;
  const rowCount = Math.ceil(games.length / columns);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 4,
  });

  // Re-measure when layout inputs change.
  useEffect(() => {
    virtualizer.measure();
  }, [virtualizer, rowHeight, columns]);

  const clampedFocus = Math.min(focusIndex, Math.max(0, games.length - 1));

  const scrollToIndex = useCallback(
    (index: number) => {
      virtualizer.scrollToIndex(Math.floor(index / columns), { align: "auto" });
    },
    [virtualizer, columns],
  );

  // Keyboard navigation (also driven by the controller via synthetic events).
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (games.length === 0) return;
      // Ignore when typing in inputs or when a dialog is open.
      const target = e.target as HTMLElement;
      if (target.closest("input, textarea, select, [role=dialog], [data-radix-popper-content-wrapper]"))
        return;

      const current = clampedFocus;
      let next = current;
      switch (e.key) {
        case "ArrowRight":
          next = Math.min(current + 1, games.length - 1);
          break;
        case "ArrowLeft":
          next = Math.max(current - 1, 0);
          break;
        case "ArrowDown":
          next = Math.min(current + columns, games.length - 1);
          break;
        case "ArrowUp":
          next = Math.max(current - columns, 0);
          break;
        case "Enter":
          e.preventDefault();
          onOpen(games[current]);
          return;
        case "f":
        case "F":
          if (e.ctrlKey || e.metaKey || e.altKey) return;
          e.preventDefault();
          onToggleFavorite(games[current]);
          return;
        case "l":
        case "L":
          if (e.ctrlKey || e.metaKey || e.altKey) return;
          e.preventDefault();
          onLaunch(games[current]);
          return;
        default:
          return;
      }
      e.preventDefault();
      setFocusIndex(next);
      scrollToIndex(next);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [games, columns, clampedFocus, onOpen, onLaunch, onToggleFavorite, setFocusIndex, scrollToIndex]);

  if (games.length === 0) return null;

  return (
    <div ref={parentRef} className="h-full overflow-y-auto px-8 pb-10" role="grid" aria-label="Game library">
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((row) => {
          const startIndex = row.index * columns;
          const rowGames = games.slice(startIndex, startIndex + columns);
          return (
            <div
              key={row.key}
              role="row"
              className="absolute left-0 top-0 w-full"
              style={{
                transform: `translateY(${row.start}px)`,
                display: viewMode === "grid" ? "grid" : "block",
                gridTemplateColumns: viewMode === "grid" ? `repeat(${columns}, 1fr)` : undefined,
                gap: viewMode === "grid" ? CARD_GAP : undefined,
              }}
            >
              {rowGames.map((game, i) => {
                const index = startIndex + i;
                return viewMode === "grid" ? (
                  <GameCard
                    key={game.id}
                    game={game}
                    platform={platformById.get(game.platformId)}
                    focused={index === clampedFocus}
                    running={runningGameIds.has(game.id)}
                    onOpen={() => {
                      setFocusIndex(index);
                      onOpen(game);
                    }}
                    onLaunch={() => onLaunch(game)}
                    onToggleFavorite={() => onToggleFavorite(game)}
                  />
                ) : (
                  <GameListRow
                    key={game.id}
                    game={game}
                    platform={platformById.get(game.platformId)}
                    focused={index === clampedFocus}
                    running={runningGameIds.has(game.id)}
                    onOpen={() => {
                      setFocusIndex(index);
                      onOpen(game);
                    }}
                    onLaunch={() => onLaunch(game)}
                    onToggleFavorite={() => onToggleFavorite(game)}
                  />
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}
