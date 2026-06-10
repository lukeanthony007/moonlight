import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "@/lib/api";
import { startGamepadNavigation } from "@/lib/gamepad";
import type { LibraryEntry, ScanProgress, ScanReport, SessionEndedPayload } from "@/lib/types";
import { useLibraryStore } from "@/stores/libraryStore";
import { useScanStore } from "@/stores/scanStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useUiStore } from "@/stores/uiStore";
import { Sidebar } from "@/components/layout/Sidebar";
import { TopBar } from "@/components/layout/TopBar";
import { AmbientBackdrop } from "@/components/layout/AmbientBackdrop";
import { ScanBanner } from "@/components/layout/ScanBanner";
import { GameDetails, GameDetailsHost } from "@/components/library/GameDetails";
import { AddGameDialog } from "@/components/dialogs/AddGameDialog";
import { MetadataEditor } from "@/components/dialogs/MetadataEditor";
import { ArtworkSelector } from "@/components/dialogs/ArtworkSelector";
import { EmulatorWizard } from "@/components/dialogs/EmulatorWizard";
import { TooltipProvider, Spinner } from "@/components/ui/misc";
import { Home } from "@/screens/Home";
import { LibraryScreen } from "@/screens/LibraryScreen";
import { Onboarding } from "@/screens/Onboarding";
import { Settings } from "@/screens/Settings";

export default function App() {
  const route = useUiStore((s) => s.route);
  const detailsGameId = useUiStore((s) => s.detailsGameId);
  const openDetails = useUiStore((s) => s.openDetails);

  const games = useLibraryStore((s) => s.games);
  const platforms = useLibraryStore((s) => s.platforms);
  const collections = useLibraryStore((s) => s.collections);
  const running = useLibraryStore((s) => s.running);
  const refresh = useLibraryStore((s) => s.refresh);
  const refreshGame = useLibraryStore((s) => s.refreshGame);
  const toggleFavorite = useLibraryStore((s) => s.toggleFavorite);
  const setHiddenAction = useLibraryStore((s) => s.setHidden);

  const settingsStore = useSettingsStore();
  const scanStore = useScanStore();

  const [bootState, setBootState] = useState<"loading" | "onboarding" | "ready">("loading");
  const [addGameOpen, setAddGameOpen] = useState(false);
  const [metadataOpen, setMetadataOpen] = useState(false);
  const [artworkOpen, setArtworkOpen] = useState(false);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [launchToast, setLaunchToast] = useState<string | null>(null);

  const detailsGame = useMemo(
    () => games.find((g) => g.id === detailsGameId) ?? null,
    [games, detailsGameId],
  );

  // Boot: load settings + library, decide onboarding.
  useEffect(() => {
    void (async () => {
      await Promise.all([settingsStore.load(), refresh(), scanStore.loadLastReport()]);
      const info = await api.getAppInfo();
      setBootState(info.onboardingComplete ? "ready" : "onboarding");
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Backend events.
  useEffect(() => {
    const unlisteners = [
      listen<ScanProgress>("scan-progress", (e) => scanStore.onProgress(e.payload)),
      listen<ScanReport>("scan-complete", (e) => {
        scanStore.onComplete(e.payload);
        void refresh();
      }),
      listen("session-started", () => {
        void api.getRunningGames().then(useLibraryStore.getState().setRunning);
      }),
      listen<SessionEndedPayload>("session-ended", (e) => {
        void api.getRunningGames().then(useLibraryStore.getState().setRunning);
        void refreshGame(e.payload.gameId);
      }),
    ];
    return () => {
      for (const u of unlisteners) void u.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Controller navigation.
  useEffect(() => startGamepadNavigation(), []);

  // Appearance settings → DOM.
  const reduceMotion = settingsStore.get("appearance.reduceMotion", false);
  const uiScale = settingsStore.get("appearance.uiScale", 1);
  const ambientEnabled = settingsStore.get("appearance.ambientBackdrop", true);
  useEffect(() => {
    document.documentElement.dataset.reduceMotion = String(reduceMotion);
    document.documentElement.style.fontSize = `${uiScale * 100}%`;
  }, [reduceMotion, uiScale]);

  const handleLaunch = useCallback(async (game: LibraryEntry) => {
    setLaunchToast(null);
    try {
      await api.launchGame(game.id);
    } catch (e) {
      setLaunchToast(api.errorMessage(e));
      setTimeout(() => setLaunchToast(null), 6000);
    }
  }, []);

  if (bootState === "loading") {
    return (
      <div className="flex h-screen items-center justify-center bg-base">
        <Spinner className="size-8" />
      </div>
    );
  }

  if (bootState === "onboarding") {
    return (
      <TooltipProvider>
        <Onboarding
          onDone={(openWizard) => {
            setBootState("ready");
            if (openWizard) setWizardOpen(true);
          }}
        />
      </TooltipProvider>
    );
  }

  const title = (() => {
    switch (route.name) {
      case "home":
        return "Home";
      case "all":
        return "All Games";
      case "recent":
        return "Recently Played";
      case "favorites":
        return "Favorites";
      case "platform":
        return platforms.find((p) => p.id === route.platformId)?.name ?? "Platform";
      case "collection":
        return collections.find((c) => c.id === route.collectionId)?.name ?? "Collection";
      case "settings":
        return "Settings";
    }
  })();

  const showTopBar = route.name !== "settings" && route.name !== "home";

  return (
    <TooltipProvider>
      <div className="flex h-screen">
        <AmbientBackdrop game={ambientEnabled ? detailsGame : null} />
        <Sidebar />
        <main className="flex min-w-0 flex-1 flex-col">
          {showTopBar && <TopBar title={title} onAddGame={() => setAddGameOpen(true)} />}
          {route.name === "home" && (
            <>
              <header className="flex items-center justify-between px-8 pb-2 pt-7">
                <h1 className="text-2xl font-bold tracking-tight">{greeting()}</h1>
              </header>
              <div className="min-h-0 flex-1">
                <Home onLaunch={handleLaunch} />
              </div>
            </>
          )}
          {route.name === "settings" && <Settings section={route.section} />}
          {route.name !== "home" && route.name !== "settings" && (
            <div className="min-h-0 flex-1">
              <LibraryScreen route={route} onLaunch={handleLaunch} />
            </div>
          )}
        </main>
      </div>

      <GameDetailsHost>
        {detailsGame && (
          <GameDetails
            key={detailsGame.id}
            game={detailsGame}
            platform={platforms.find((p) => p.id === detailsGame.platformId)}
            running={running.some((r) => r.gameId === detailsGame.id)}
            onClose={() => openDetails(null)}
            onToggleFavorite={() => void toggleFavorite(detailsGame.id)}
            onToggleHidden={() => void setHiddenAction(detailsGame.id, !detailsGame.hidden)}
            onEditMetadata={() => setMetadataOpen(true)}
            onEditArtwork={() => setArtworkOpen(true)}
            onDelete={async () => {
              if (!confirm(`Remove “${detailsGame.title}” from your library? Files on disk are not touched.`))
                return;
              await api.deleteGame(detailsGame.id);
              openDetails(null);
              await refresh();
            }}
          />
        )}
      </GameDetailsHost>

      {detailsGame && (
        <MetadataEditor game={detailsGame} open={metadataOpen} onOpenChange={setMetadataOpen} />
      )}
      {detailsGame && (
        <ArtworkSelector game={detailsGame} open={artworkOpen} onOpenChange={setArtworkOpen} />
      )}
      <AddGameDialog open={addGameOpen} onOpenChange={setAddGameOpen} onAdded={(id) => openDetails(id)} />
      <EmulatorWizard open={wizardOpen} onOpenChange={setWizardOpen} />
      <ScanBanner />

      {launchToast && (
        <div
          role="alert"
          className="glass-strong card-shadow fixed bottom-5 right-5 z-50 max-w-md rounded-2xl border-danger/30 bg-[#1a1218]/95 px-5 py-4 text-sm text-danger animate-fade-in"
        >
          {launchToast}
        </div>
      )}
    </TooltipProvider>
  );
}

function greeting(): string {
  const hour = new Date().getHours();
  if (hour < 5) return "Up late?";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}
