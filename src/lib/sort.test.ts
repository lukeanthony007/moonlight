import { describe, expect, it } from "vitest";
import { DEFAULT_FILTER, allGenres, filterGames, sortGames } from "./sort";
import type { Installation, LibraryEntry } from "./types";

let counter = 0;
function install(overrides: Partial<Installation> = {}): Installation {
  return {
    id: `i${counter++}`,
    gameId: "g",
    sourceType: "rom",
    sourceId: null,
    path: null,
    emulatorId: null,
    launchArgs: null,
    workingDirectory: null,
    installed: true,
    fileSize: null,
    ...overrides,
  };
}

function game(overrides: Partial<LibraryEntry> = {}): LibraryEntry {
  const title = overrides.title ?? `Game ${counter++}`;
  return {
    id: `id-${title}`,
    title,
    sortTitle: title.toLowerCase().replace(/^the /, ""),
    description: null,
    releaseDate: null,
    developer: null,
    publisher: null,
    genres: [],
    region: null,
    platformId: "nes",
    series: null,
    favorite: false,
    hidden: false,
    dateAdded: "2026-01-01T00:00:00Z",
    lastPlayed: null,
    playtimeSeconds: 0,
    userRating: null,
    lockedFields: [],
    providerMetadata: null,
    installations: [install()],
    artwork: {},
    ...overrides,
  };
}

describe("filterGames", () => {
  it("hides hidden games unless requested", () => {
    const games = [game({ title: "Visible" }), game({ title: "Secret", hidden: true })];
    expect(filterGames(games, DEFAULT_FILTER).map((g) => g.title)).toEqual(["Visible"]);
    expect(filterGames(games, { ...DEFAULT_FILTER, showHidden: true })).toHaveLength(2);
  });

  it("searches title, developer and publisher", () => {
    const games = [
      game({ title: "Super Metroid", developer: "Nintendo" }),
      game({ title: "Sonic", publisher: "Sega" }),
    ];
    expect(filterGames(games, { ...DEFAULT_FILTER, search: "metroid" })).toHaveLength(1);
    expect(filterGames(games, { ...DEFAULT_FILTER, search: "sega" })).toHaveLength(1);
    expect(filterGames(games, { ...DEFAULT_FILTER, search: "zelda" })).toHaveLength(0);
  });

  it("filters by platform, favorites, installed state and source", () => {
    const games = [
      game({ title: "A", platformId: "snes", favorite: true }),
      game({ title: "B", platformId: "nes", installations: [install({ installed: false })] }),
      game({ title: "C", platformId: "nes", installations: [install({ sourceType: "steam" })] }),
    ];
    expect(filterGames(games, { ...DEFAULT_FILTER, platformId: "snes" }).map((g) => g.title)).toEqual(["A"]);
    expect(filterGames(games, { ...DEFAULT_FILTER, favoritesOnly: true }).map((g) => g.title)).toEqual(["A"]);
    expect(filterGames(games, { ...DEFAULT_FILTER, installedOnly: true }).map((g) => g.title)).toEqual(["A", "C"]);
    expect(filterGames(games, { ...DEFAULT_FILTER, sourceType: "steam" }).map((g) => g.title)).toEqual(["C"]);
  });

  it("filters by genre and collection", () => {
    const games = [game({ title: "A", genres: ["RPG"] }), game({ title: "B", genres: ["Action"] })];
    expect(filterGames(games, { ...DEFAULT_FILTER, genre: "RPG" }).map((g) => g.title)).toEqual(["A"]);
    expect(
      filterGames(games, { ...DEFAULT_FILTER, collectionGameIds: ["id-B"] }).map((g) => g.title),
    ).toEqual(["B"]);
  });
});

describe("sortGames", () => {
  it("sorts by title using sort title (articles stripped)", () => {
    const games = [game({ title: "Zelda" }), game({ title: "The Adventure" })];
    expect(sortGames(games, "title").map((g) => g.title)).toEqual(["The Adventure", "Zelda"]);
  });

  it("sorts release dates newest first with nulls last", () => {
    const games = [
      game({ title: "Old", releaseDate: "1990-05-01" }),
      game({ title: "Unknown", releaseDate: null }),
      game({ title: "New", releaseDate: "2005-11-18" }),
    ];
    expect(sortGames(games, "releaseNewest").map((g) => g.title)).toEqual(["New", "Old", "Unknown"]);
    expect(sortGames(games, "releaseOldest").map((g) => g.title)).toEqual(["Old", "New", "Unknown"]);
  });

  it("sorts by recently played with never-played last", () => {
    const games = [
      game({ title: "Never", lastPlayed: null }),
      game({ title: "Today", lastPlayed: "2026-06-10T10:00:00Z" }),
      game({ title: "LastWeek", lastPlayed: "2026-06-03T10:00:00Z" }),
    ];
    expect(sortGames(games, "recentlyPlayed").map((g) => g.title)).toEqual(["Today", "LastWeek", "Never"]);
  });

  it("sorts by most played and rating", () => {
    const games = [
      game({ title: "A", playtimeSeconds: 50, userRating: 2 }),
      game({ title: "B", playtimeSeconds: 500, userRating: null }),
      game({ title: "C", playtimeSeconds: 5, userRating: 5 }),
    ];
    expect(sortGames(games, "mostPlayed").map((g) => g.title)).toEqual(["B", "A", "C"]);
    expect(sortGames(games, "userRating").map((g) => g.title)).toEqual(["C", "A", "B"]);
  });

  it("does not mutate the input array", () => {
    const games = [game({ title: "B" }), game({ title: "A" })];
    const original = [...games];
    sortGames(games, "title");
    expect(games).toEqual(original);
  });
});

describe("allGenres", () => {
  it("returns sorted distinct genres", () => {
    const games = [game({ genres: ["RPG", "Action"] }), game({ genres: ["Action"] })];
    expect(allGenres(games)).toEqual(["Action", "RPG"]);
  });
});
