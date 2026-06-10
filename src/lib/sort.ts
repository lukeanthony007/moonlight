/** Pure filtering and sorting over library entries. Kept free of React and
 * Tauri imports so it is trivially unit-testable. */

import type { LibraryEntry } from "./types";

export type SortMode =
  | "title"
  | "releaseNewest"
  | "releaseOldest"
  | "recentlyPlayed"
  | "recentlyAdded"
  | "mostPlayed"
  | "userRating"
  | "platform";

export const SORT_LABELS: Record<SortMode, string> = {
  title: "Title",
  releaseNewest: "Release date · newest",
  releaseOldest: "Release date · oldest",
  recentlyPlayed: "Recently played",
  recentlyAdded: "Recently added",
  mostPlayed: "Most played",
  userRating: "Your rating",
  platform: "Platform",
};

export interface LibraryFilter {
  search: string;
  platformId: string | null;
  sourceType: string | null;
  genre: string | null;
  favoritesOnly: boolean;
  installedOnly: boolean;
  showHidden: boolean;
  collectionGameIds: string[] | null;
}

export const DEFAULT_FILTER: LibraryFilter = {
  search: "",
  platformId: null,
  sourceType: null,
  genre: null,
  favoritesOnly: false,
  installedOnly: false,
  showHidden: false,
  collectionGameIds: null,
};

export function filterGames(games: LibraryEntry[], filter: LibraryFilter): LibraryEntry[] {
  const search = filter.search.trim().toLowerCase();
  return games.filter((game) => {
    if (!filter.showHidden && game.hidden) return false;
    if (filter.favoritesOnly && !game.favorite) return false;
    if (filter.platformId && game.platformId !== filter.platformId) return false;
    if (filter.sourceType && !game.installations.some((i) => i.sourceType === filter.sourceType))
      return false;
    if (filter.installedOnly && !game.installations.some((i) => i.installed)) return false;
    if (filter.genre && !game.genres.includes(filter.genre)) return false;
    if (filter.collectionGameIds && !filter.collectionGameIds.includes(game.id)) return false;
    if (search) {
      const haystack = `${game.title} ${game.developer ?? ""} ${game.publisher ?? ""} ${game.series ?? ""}`.toLowerCase();
      if (!haystack.includes(search)) return false;
    }
    return true;
  });
}

function compareNullableDesc(a: string | null, b: string | null): number {
  if (a === b) return 0;
  if (a === null) return 1; // nulls sink to the bottom
  if (b === null) return -1;
  return b.localeCompare(a);
}

export function sortGames(games: LibraryEntry[], mode: SortMode): LibraryEntry[] {
  const byTitle = (a: LibraryEntry, b: LibraryEntry) => a.sortTitle.localeCompare(b.sortTitle);
  const sorted = [...games];
  switch (mode) {
    case "title":
      sorted.sort(byTitle);
      break;
    case "releaseNewest":
      sorted.sort((a, b) => compareNullableDesc(a.releaseDate, b.releaseDate) || byTitle(a, b));
      break;
    case "releaseOldest":
      sorted.sort((a, b) => {
        if (a.releaseDate === b.releaseDate) return byTitle(a, b);
        if (a.releaseDate === null) return 1;
        if (b.releaseDate === null) return -1;
        return a.releaseDate.localeCompare(b.releaseDate) || byTitle(a, b);
      });
      break;
    case "recentlyPlayed":
      sorted.sort((a, b) => compareNullableDesc(a.lastPlayed, b.lastPlayed) || byTitle(a, b));
      break;
    case "recentlyAdded":
      sorted.sort((a, b) => compareNullableDesc(a.dateAdded, b.dateAdded) || byTitle(a, b));
      break;
    case "mostPlayed":
      sorted.sort((a, b) => b.playtimeSeconds - a.playtimeSeconds || byTitle(a, b));
      break;
    case "userRating":
      sorted.sort((a, b) => (b.userRating ?? -1) - (a.userRating ?? -1) || byTitle(a, b));
      break;
    case "platform":
      sorted.sort((a, b) => a.platformId.localeCompare(b.platformId) || byTitle(a, b));
      break;
  }
  return sorted;
}

/** Distinct genres across the library, for the filter dropdown. */
export function allGenres(games: LibraryEntry[]): string[] {
  const set = new Set<string>();
  for (const g of games) for (const genre of g.genres) set.add(genre);
  return [...set].sort();
}
