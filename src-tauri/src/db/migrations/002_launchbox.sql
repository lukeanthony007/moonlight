-- Local cache of the LaunchBox Games Database (keyless retro metadata).
-- Populated by downloading and stream-parsing the official Metadata.zip;
-- queried during enrichment to fill descriptions, developer, publisher,
-- genres and release dates for console ROM games.
CREATE TABLE launchbox_games (
  platform_id TEXT NOT NULL,
  match_key TEXT NOT NULL,
  name TEXT NOT NULL,
  release_date TEXT,
  overview TEXT,
  developer TEXT,
  publisher TEXT,
  genres TEXT,
  rating REAL
);
CREATE INDEX idx_launchbox_match ON launchbox_games(platform_id, match_key);
