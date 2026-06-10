import { create } from "zustand";
import * as api from "@/lib/api";
import type { Collection, Emulator, LibraryEntry, Platform, RomDirectory, RunningGame } from "@/lib/types";

interface LibraryState {
  games: LibraryEntry[];
  platforms: Platform[];
  collections: Collection[];
  emulators: Emulator[];
  romDirectories: RomDirectory[];
  running: RunningGame[];
  loaded: boolean;
  loading: boolean;
  error: string | null;

  refresh: () => Promise<void>;
  refreshGame: (gameId: string) => Promise<void>;
  toggleFavorite: (gameId: string) => Promise<void>;
  setHidden: (gameId: string, hidden: boolean) => Promise<void>;
  setRunning: (running: RunningGame[]) => void;
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  games: [],
  platforms: [],
  collections: [],
  emulators: [],
  romDirectories: [],
  running: [],
  loaded: false,
  loading: false,
  error: null,

  refresh: async () => {
    set({ loading: true, error: null });
    try {
      const [games, platforms, collections, emulators, romDirectories, running] = await Promise.all([
        api.listLibrary(),
        api.listPlatforms(),
        api.listCollections(),
        api.listEmulators(),
        api.listRomDirectories(),
        api.getRunningGames(),
      ]);
      set({ games, platforms, collections, emulators, romDirectories, running, loaded: true, loading: false });
    } catch (e) {
      set({ error: api.errorMessage(e), loading: false, loaded: true });
    }
  },

  refreshGame: async (gameId) => {
    try {
      const entry = await api.getGame(gameId);
      set({ games: get().games.map((g) => (g.id === gameId ? entry : g)) });
    } catch {
      // Game may have been deleted; fall back to full refresh.
      void get().refresh();
    }
  },

  toggleFavorite: async (gameId) => {
    const game = get().games.find((g) => g.id === gameId);
    if (!game) return;
    const favorite = !game.favorite;
    // Optimistic update; revert on failure.
    set({ games: get().games.map((g) => (g.id === gameId ? { ...g, favorite } : g)) });
    try {
      await api.setFavorite(gameId, favorite);
    } catch {
      set({ games: get().games.map((g) => (g.id === gameId ? { ...g, favorite: !favorite } : g)) });
    }
  },

  setHidden: async (gameId, hidden) => {
    set({ games: get().games.map((g) => (g.id === gameId ? { ...g, hidden } : g)) });
    try {
      await api.setHidden(gameId, hidden);
    } catch {
      void get().refreshGame(gameId);
    }
  },

  setRunning: (running) => set({ running }),
}));

/** Platforms that actually contain games, for the sidebar. */
export function platformsWithGames(games: LibraryEntry[], platforms: Platform[]): (Platform & { count: number })[] {
  const counts = new Map<string, number>();
  for (const game of games) {
    if (game.hidden) continue;
    counts.set(game.platformId, (counts.get(game.platformId) ?? 0) + 1);
  }
  return platforms
    .filter((p) => counts.has(p.id))
    .map((p) => ({ ...p, count: counts.get(p.id) ?? 0 }));
}
