CREATE TABLE platforms (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  short_name TEXT NOT NULL,
  manufacturer TEXT,
  default_emulator_id TEXT,
  extensions TEXT NOT NULL DEFAULT '[]',
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE emulators (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  executable_path TEXT NOT NULL,
  emulator_type TEXT NOT NULL,
  platforms TEXT NOT NULL DEFAULT '[]',
  command_template TEXT NOT NULL DEFAULT '{executable} {gamePath}',
  core_name TEXT,
  working_directory TEXT,
  environment TEXT NOT NULL DEFAULT '{}',
  enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE games (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  sort_title TEXT NOT NULL,
  description TEXT,
  release_date TEXT,
  developer TEXT,
  publisher TEXT,
  genres TEXT NOT NULL DEFAULT '[]',
  region TEXT,
  platform_id TEXT NOT NULL REFERENCES platforms(id),
  series TEXT,
  favorite INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  date_added TEXT NOT NULL,
  last_played TEXT,
  playtime_seconds INTEGER NOT NULL DEFAULT 0,
  user_rating INTEGER,
  locked_fields TEXT NOT NULL DEFAULT '[]',
  provider_metadata TEXT
);
CREATE INDEX idx_games_platform ON games(platform_id);
CREATE INDEX idx_games_sort_title ON games(sort_title);

CREATE TABLE installations (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  source_type TEXT NOT NULL,
  source_id TEXT,
  path TEXT,
  emulator_id TEXT,
  launch_args TEXT,
  working_directory TEXT,
  installed INTEGER NOT NULL DEFAULT 1,
  file_size INTEGER
);
CREATE INDEX idx_installations_game ON installations(game_id);
CREATE UNIQUE INDEX idx_installations_identity
  ON installations(source_type, source_id) WHERE source_id IS NOT NULL;

CREATE TABLE artwork (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  local_path TEXT,
  remote_url TEXT,
  provider TEXT,
  user_selected INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_artwork_game ON artwork(game_id);

CREATE TABLE play_sessions (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  duration_seconds INTEGER,
  exit_status TEXT
);
CREATE INDEX idx_sessions_game ON play_sessions(game_id);
CREATE INDEX idx_sessions_started ON play_sessions(started_at);

CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE collection_games (
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  PRIMARY KEY (collection_id, game_id)
);

CREATE TABLE rom_directories (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL,
  platform_id TEXT NOT NULL REFERENCES platforms(id),
  emulator_id TEXT,
  enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE provider_cache (
  cache_key TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  fetched_at TEXT NOT NULL
);
