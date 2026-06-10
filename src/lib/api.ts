/** Typed wrappers around the Tauri command surface. */

import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  AppErrorPayload,
  AppInfo,
  Artwork,
  ArtworkCandidate,
  ArtworkKind,
  Collection,
  DetectedCore,
  Diagnostics,
  Emulator,
  EmulatorPreset,
  Game,
  GamePatch,
  LibraryEntry,
  ManualGameInput,
  Platform,
  PlaySession,
  ProviderMatch,
  ProviderStatus,
  RomDirectory,
  RunningGame,
  ScanReport,
  ScanScopeInput,
  SearchQuery,
} from "./types";

export function isAppError(e: unknown): e is AppErrorPayload {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Convert a local artwork path into a webview-loadable URL. */
export function artworkUrl(localPath: string | undefined | null): string | undefined {
  return localPath ? convertFileSrc(localPath) : undefined;
}

// Library
export const listLibrary = () => invoke<LibraryEntry[]>("list_library");
export const getGame = (gameId: string) => invoke<LibraryEntry>("get_game", { gameId });
export const updateGame = (gameId: string, patch: GamePatch) =>
  invoke<Game>("update_game", { gameId, patch });
export const setFavorite = (gameId: string, favorite: boolean) =>
  invoke<void>("set_favorite", { gameId, favorite });
export const setHidden = (gameId: string, hidden: boolean) =>
  invoke<void>("set_hidden", { gameId, hidden });
export const deleteGame = (gameId: string) => invoke<void>("delete_game", { gameId });
export const restoreProviderMetadata = (gameId: string) =>
  invoke<Game>("restore_provider_metadata", { gameId });
export const addManualGame = (input: ManualGameInput) =>
  invoke<LibraryEntry>("add_manual_game", { input });
export const updateInstallation = (patch: {
  installationId: string;
  path?: string | null;
  emulatorId?: string | null;
  launchArgs?: string | null;
  workingDirectory?: string | null;
}) => invoke<void>("update_installation", { patch });
export const listPlatforms = () => invoke<Platform[]>("list_platforms");
export const setPlatformDefaultEmulator = (platformId: string, emulatorId: string | null) =>
  invoke<void>("set_platform_default_emulator", { platformId, emulatorId });
export const listSessions = (gameId: string, limit?: number) =>
  invoke<PlaySession[]>("list_sessions", { gameId, limit });

// Collections
export const listCollections = () => invoke<Collection[]>("list_collections");
export const createCollection = (name: string) => invoke<Collection>("create_collection", { name });
export const renameCollection = (collectionId: string, name: string) =>
  invoke<void>("rename_collection", { collectionId, name });
export const deleteCollection = (collectionId: string) =>
  invoke<void>("delete_collection", { collectionId });
export const addGameToCollection = (collectionId: string, gameId: string) =>
  invoke<void>("add_game_to_collection", { collectionId, gameId });
export const removeGameFromCollection = (collectionId: string, gameId: string) =>
  invoke<void>("remove_game_from_collection", { collectionId, gameId });

// Emulators
export const listEmulators = () => invoke<Emulator[]>("list_emulators");
export const saveEmulator = (emulator: Emulator) => invoke<Emulator>("save_emulator", { emulator });
export const deleteEmulator = (emulatorId: string) => invoke<void>("delete_emulator", { emulatorId });
export const testEmulatorConfig = (emulator: Emulator) =>
  invoke<string>("test_emulator_config", { emulator });
export const getEmulatorPresets = () => invoke<EmulatorPreset[]>("get_emulator_presets");
export const detectRetroarchCores = (customDir?: string) =>
  invoke<DetectedCore[]>("detect_retroarch_cores", { customDir: customDir ?? null });
export const detectEmulatorExecutable = (presetId: string) =>
  invoke<string | null>("detect_emulator_executable", { presetId });
export const listRomDirectories = () => invoke<RomDirectory[]>("list_rom_directories");
export const saveRomDirectory = (directory: RomDirectory) =>
  invoke<RomDirectory>("save_rom_directory", { directory });
export const deleteRomDirectory = (directoryId: string) =>
  invoke<void>("delete_rom_directory", { directoryId });

// Launching
export const launchGame = (gameId: string) => invoke<RunningGame>("launch_game", { gameId });
export const getRunningGames = () => invoke<RunningGame[]>("get_running_games");

// Scanning
export const startScan = (scope: ScanScopeInput) => invoke<string>("start_scan", { scope });
export const cancelScan = (scanId: string) => invoke<boolean>("cancel_scan", { scanId });
export const getLastScanReport = () => invoke<ScanReport | null>("get_last_scan_report");

// Metadata & artwork
export const getProviderStatuses = () => invoke<ProviderStatus[]>("get_provider_statuses");
export const searchMetadata = (providerId: string, query: SearchQuery) =>
  invoke<ProviderMatch[]>("search_metadata", { providerId, query });
export const getArtworkCandidates = (providerId: string, providerGameId: string, kind: ArtworkKind) =>
  invoke<ArtworkCandidate[]>("get_artwork_candidates", { providerId, providerGameId, kind });
export const applyProviderMatch = (
  gameId: string,
  providerId: string,
  providerGameId: string,
  fetchArtwork: boolean,
) => invoke<Game>("apply_provider_match", { gameId, providerId, providerGameId, fetchArtwork });
export const listArtwork = (gameId: string) => invoke<Artwork[]>("list_artwork", { gameId });
export const importArtworkFile = (gameId: string, kind: ArtworkKind, path: string) =>
  invoke<Artwork>("import_artwork_file", { gameId, kind, path });
export const downloadArtwork = (
  gameId: string,
  kind: ArtworkKind,
  url: string,
  provider: string | null,
  select: boolean,
) => invoke<Artwork>("download_artwork", { gameId, kind, url, provider, select });
export const selectArtwork = (artworkId: string) => invoke<void>("select_artwork", { artworkId });
export const deleteArtwork = (artworkId: string) => invoke<void>("delete_artwork", { artworkId });

// Settings & diagnostics
export const getSettings = () => invoke<Record<string, unknown>>("get_settings");
export const setSetting = (key: string, value: unknown) => invoke<void>("set_setting", { key, value });
export const getAppInfo = () => invoke<AppInfo>("get_app_info");
export const completeOnboarding = () => invoke<void>("complete_onboarding");
export const backupDatabase = (destPath: string) => invoke<string>("backup_database", { destPath });
export const exportLibrary = (destPath: string) => invoke<string>("export_library", { destPath });
export const getDiagnostics = () => invoke<Diagnostics>("get_diagnostics");
export const readLogs = (lines?: number) => invoke<string[]>("read_logs", { lines });
