import { useMemo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Heart, SearchX } from "lucide-react";
import { DEFAULT_FILTER, filterGames, sortGames } from "@/lib/sort";
import type { LibraryEntry } from "@/lib/types";
import { useLibraryStore } from "@/stores/libraryStore";
import { useUiStore, type Route } from "@/stores/uiStore";
import { GameGrid } from "@/components/library/GameGrid";
import { EmptyState } from "@/components/library/EmptyState";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/misc";

interface LibraryScreenProps {
  route: Route;
  onLaunch: (game: LibraryEntry) => void;
}

/** Shared screen for All Games / Recently Played / Favorites / Platform /
 * Collection — they differ only in their base filter. */
export function LibraryScreen({ route, onLaunch }: LibraryScreenProps) {
  const games = useLibraryStore((s) => s.games);
  const platforms = useLibraryStore((s) => s.platforms);
  const collections = useLibraryStore((s) => s.collections);
  const running = useLibraryStore((s) => s.running);
  const loading = useLibraryStore((s) => s.loading);
  const loaded = useLibraryStore((s) => s.loaded);
  const toggleFavorite = useLibraryStore((s) => s.toggleFavorite);

  const search = useUiStore((s) => s.search);
  const sortMode = useUiStore((s) => s.sortMode);
  const viewMode = useUiStore((s) => s.viewMode);
  const installedOnly = useUiStore((s) => s.installedOnly);
  const showHidden = useUiStore((s) => s.showHidden);
  const genre = useUiStore((s) => s.genre);
  const sourceType = useUiStore((s) => s.sourceType);
  const openDetails = useUiStore((s) => s.openDetails);
  const navigate = useUiStore((s) => s.navigate);

  const filtered = useMemo(() => {
    const filter = {
      ...DEFAULT_FILTER,
      search,
      installedOnly,
      showHidden,
      genre,
      sourceType,
      favoritesOnly: route.name === "favorites",
      platformId: route.name === "platform" ? route.platformId : null,
      collectionGameIds:
        route.name === "collection"
          ? (collections.find((c) => c.id === route.collectionId)?.gameIds ?? [])
          : null,
    };
    let result = filterGames(games, filter);
    if (route.name === "recent") {
      result = result.filter((g) => g.lastPlayed !== null);
    }
    return sortGames(result, sortMode);
  }, [games, collections, route, search, sortMode, installedOnly, showHidden, genre, sourceType]);

  const runningIds = useMemo(() => new Set(running.map((r) => r.gameId)), [running]);

  if (!loaded || loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner className="size-8" />
      </div>
    );
  }

  if (filtered.length === 0) {
    if (search || genre || sourceType || installedOnly) {
      return (
        <EmptyState
          icon={<SearchX className="size-9 text-ink-faint" />}
          title="No games match"
          description="Try a different search or clear the active filters."
        />
      );
    }
    if (route.name === "favorites") {
      return (
        <EmptyState
          icon={<Heart className="size-9 text-ink-faint" />}
          title="No favorites yet"
          description="Open a game and tap the heart — or press F with a game focused — to add it here."
        />
      );
    }
    if (route.name === "recent") {
      return (
        <EmptyState
          title="Nothing played yet"
          description="Games you launch will show up here, sorted by when you last played them."
        />
      );
    }
    return (
      <EmptyState
        title="No games here yet"
        description="Connect an emulator with game directories, or import your Steam library."
        action={
          <Button onClick={() => navigate({ name: "settings", section: "emulators" })}>
            Open emulator settings
          </Button>
        }
      />
    );
  }

  // Crossfade between routes.
  const routeId =
    route.name === "platform"
      ? `platform:${route.platformId}`
      : route.name === "collection"
        ? `collection:${route.collectionId}`
        : route.name;

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={routeId}
        className="h-full"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.18 }}
      >
        <GameGrid
          games={filtered}
          platforms={platforms}
          runningGameIds={runningIds}
          viewMode={viewMode}
          onOpen={(game) => openDetails(game.id)}
          onLaunch={onLaunch}
          onToggleFavorite={(game) => void toggleFavorite(game.id)}
        />
      </motion.div>
    </AnimatePresence>
  );
}
