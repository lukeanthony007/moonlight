import { create } from "zustand";
import type { SortMode } from "@/lib/sort";

export type Route =
  | { name: "home" }
  | { name: "all" }
  | { name: "recent" }
  | { name: "favorites" }
  | { name: "platform"; platformId: string }
  | { name: "collection"; collectionId: string }
  | { name: "settings"; section: SettingsSection };

export type SettingsSection =
  | "general"
  | "libraries"
  | "emulators"
  | "platforms"
  | "providers"
  | "artwork"
  | "appearance"
  | "input"
  | "importExport"
  | "diagnostics";

export type ViewMode = "grid" | "list";

interface UiState {
  route: Route;
  /** Game whose details drawer is open. */
  detailsGameId: string | null;
  viewMode: ViewMode;
  search: string;
  sortMode: SortMode;
  /** Remembered sort mode per platform view. */
  platformSortModes: Record<string, SortMode>;
  installedOnly: boolean;
  showHidden: boolean;
  genre: string | null;
  sourceType: string | null;
  /** Index of the keyboard/controller-focused card in the current grid. */
  focusIndex: number;

  navigate: (route: Route) => void;
  openDetails: (gameId: string | null) => void;
  setViewMode: (mode: ViewMode) => void;
  setSearch: (search: string) => void;
  setSortMode: (mode: SortMode) => void;
  setInstalledOnly: (v: boolean) => void;
  setShowHidden: (v: boolean) => void;
  setGenre: (genre: string | null) => void;
  setSourceType: (source: string | null) => void;
  setFocusIndex: (index: number) => void;
}

const SORT_STORAGE_KEY = "moonlight.platformSortModes";

function loadPlatformSorts(): Record<string, SortMode> {
  try {
    return JSON.parse(localStorage.getItem(SORT_STORAGE_KEY) ?? "{}") as Record<string, SortMode>;
  } catch {
    return {};
  }
}

export const useUiStore = create<UiState>((set, get) => ({
  route: { name: "home" },
  detailsGameId: null,
  viewMode: "grid",
  search: "",
  sortMode: "title",
  platformSortModes: loadPlatformSorts(),
  installedOnly: false,
  showHidden: false,
  genre: null,
  sourceType: null,
  focusIndex: 0,

  navigate: (route) => {
    const state = get();
    // Platform views remember their last sort mode.
    let sortMode = state.sortMode;
    if (route.name === "platform") {
      sortMode = state.platformSortModes[route.platformId] ?? "title";
    } else if (route.name === "recent") {
      sortMode = "recentlyPlayed";
    }
    set({ route, search: "", focusIndex: 0, detailsGameId: null, genre: null, sourceType: null, sortMode });
  },

  openDetails: (gameId) => set({ detailsGameId: gameId }),
  setViewMode: (viewMode) => set({ viewMode }),
  setSearch: (search) => set({ search, focusIndex: 0 }),

  setSortMode: (sortMode) => {
    const state = get();
    if (state.route.name === "platform") {
      const platformSortModes = { ...state.platformSortModes, [state.route.platformId]: sortMode };
      localStorage.setItem(SORT_STORAGE_KEY, JSON.stringify(platformSortModes));
      set({ sortMode, platformSortModes });
    } else {
      set({ sortMode });
    }
  },

  setInstalledOnly: (installedOnly) => set({ installedOnly, focusIndex: 0 }),
  setShowHidden: (showHidden) => set({ showHidden, focusIndex: 0 }),
  setGenre: (genre) => set({ genre, focusIndex: 0 }),
  setSourceType: (sourceType) => set({ sourceType, focusIndex: 0 }),
  setFocusIndex: (focusIndex) => set({ focusIndex }),
}));
