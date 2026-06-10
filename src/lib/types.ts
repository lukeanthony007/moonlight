/** TypeScript mirrors of the Rust domain models (camelCase via serde). */

export interface Game {
  id: string;
  title: string;
  sortTitle: string;
  description: string | null;
  releaseDate: string | null;
  developer: string | null;
  publisher: string | null;
  genres: string[];
  region: string | null;
  platformId: string;
  series: string | null;
  favorite: boolean;
  hidden: boolean;
  dateAdded: string;
  lastPlayed: string | null;
  playtimeSeconds: number;
  userRating: number | null;
  lockedFields: string[];
  providerMetadata: Record<string, unknown> | null;
}

export interface Installation {
  id: string;
  gameId: string;
  sourceType: "steam" | "rom" | "manual";
  sourceId: string | null;
  path: string | null;
  emulatorId: string | null;
  launchArgs: string | null;
  workingDirectory: string | null;
  installed: boolean;
  fileSize: number | null;
}

export type ArtworkKind = "boxart" | "background" | "screenshot" | "logo" | "icon" | "banner";

export interface LibraryEntry extends Game {
  installations: Installation[];
  artwork: Partial<Record<ArtworkKind, string>>;
}

export interface Platform {
  id: string;
  name: string;
  shortName: string;
  manufacturer: string | null;
  defaultEmulatorId: string | null;
  extensions: string[];
  sortOrder: number;
}

export interface Emulator {
  id: string;
  name: string;
  executablePath: string;
  emulatorType: "retroarch" | "standalone" | "custom";
  platforms: string[];
  commandTemplate: string;
  coreName: string | null;
  workingDirectory: string | null;
  environment: Record<string, string>;
  enabled: boolean;
}

export interface EmulatorPreset {
  id: string;
  name: string;
  emulatorType: Emulator["emulatorType"];
  platforms: string[];
  commandTemplate: string;
  executableHints: string[];
  extensions: string[];
}

export interface DetectedCore {
  path: string;
  fileName: string;
  displayName: string;
  platformId: string | null;
}

export interface Artwork {
  id: string;
  gameId: string;
  kind: ArtworkKind;
  localPath: string | null;
  remoteUrl: string | null;
  provider: string | null;
  userSelected: boolean;
}

export interface PlaySession {
  id: string;
  gameId: string;
  startedAt: string;
  endedAt: string | null;
  durationSeconds: number | null;
  exitStatus: string | null;
}

export interface Collection {
  id: string;
  name: string;
  createdAt: string;
  sortOrder: number;
  gameIds: string[];
}

export interface RomDirectory {
  id: string;
  path: string;
  platformId: string;
  emulatorId: string | null;
  enabled: boolean;
}

export interface ScanReport {
  scanId: string;
  source: string;
  added: number;
  updated: number;
  missing: number;
  reconnected: number;
  skipped: number;
  errors: string[];
  durationMs: number;
  cancelled: boolean;
}

export interface ScanProgress {
  scanId: string;
  source: string;
  phase: string;
  current: number;
  total: number;
  message: string;
}

export interface RunningGame {
  gameId: string;
  sessionId: string;
  startedAt: string;
}

export interface SessionEndedPayload {
  gameId: string;
  sessionId: string;
  durationSeconds: number;
  exitStatus: string;
}

export interface SearchQuery {
  title: string;
  platformId: string | null;
  region: string | null;
  releaseYear: number | null;
}

export interface ProviderMatch {
  provider: string;
  providerGameId: string;
  title: string;
  releaseYear: number | null;
  confidence: number;
}

export interface ArtworkCandidate {
  provider: string;
  kind: ArtworkKind;
  url: string;
  thumbnailUrl: string | null;
  width: number | null;
  height: number | null;
  author: string | null;
}

export interface ProviderStatus {
  id: string;
  name: string;
  configured: boolean;
  requiresApiKey: boolean;
  attribution: string;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  dbPath: string;
  artworkDir: string;
  logDir: string;
  onboardingComplete: boolean;
}

export interface Diagnostics {
  version: string;
  counts: Record<string, number>;
  dataDir: string;
  dbPath: string;
  logDir: string;
  dbSizeBytes: number;
  artworkSizeBytes: number;
}

export interface GamePatch {
  title?: string;
  sortTitle?: string;
  description?: string | null;
  releaseDate?: string | null;
  developer?: string | null;
  publisher?: string | null;
  genres?: string[];
  region?: string | null;
  series?: string | null;
  userRating?: number | null;
  lockedFields?: string[];
}

export interface ManualGameInput {
  title: string;
  platformId: string;
  path?: string | null;
  emulatorId?: string | null;
  launchArgs?: string | null;
  workingDirectory?: string | null;
}

export type ScanScopeInput =
  | { kind: "all" }
  | { kind: "steam" }
  | { kind: "directory"; directoryId: string }
  | { kind: "platform"; platformId: string };

export interface AppErrorPayload {
  kind: string;
  message: string;
}
