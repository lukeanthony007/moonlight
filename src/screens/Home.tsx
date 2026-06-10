import { useMemo } from "react";
import { motion } from "framer-motion";
import { ArrowRight, Clock, Gamepad2, Heart, Layers, Play, Sparkles } from "lucide-react";
import { artworkUrl } from "@/lib/api";
import type { LibraryEntry } from "@/lib/types";
import { sortGames } from "@/lib/sort";
import { cn, formatPlaytime, formatRelativeTime } from "@/lib/utils";
import { platformsWithGames, useLibraryStore } from "@/stores/libraryStore";
import { useUiStore } from "@/stores/uiStore";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/library/EmptyState";

export function Home({ onLaunch }: { onLaunch: (game: LibraryEntry) => void }) {
  const games = useLibraryStore((s) => s.games);
  const platforms = useLibraryStore((s) => s.platforms);
  const navigate = useUiStore((s) => s.navigate);
  const openDetails = useUiStore((s) => s.openDetails);

  const visible = useMemo(() => games.filter((g) => !g.hidden), [games]);
  const continuePlaying = useMemo(
    () => sortGames(visible, "recentlyPlayed").filter((g) => g.lastPlayed).slice(0, 6),
    [visible],
  );
  const recentlyAdded = useMemo(() => sortGames(visible, "recentlyAdded").slice(0, 6), [visible]);
  const favorites = useMemo(() => sortGames(visible.filter((g) => g.favorite), "title").slice(0, 6), [visible]);
  const sidebarPlatforms = useMemo(() => platformsWithGames(games, platforms), [games, platforms]);

  const totalPlaytime = useMemo(() => visible.reduce((sum, g) => sum + g.playtimeSeconds, 0), [visible]);
  const hero = continuePlaying[0] ?? recentlyAdded[0];

  if (visible.length === 0) {
    return (
      <EmptyState
        title="Welcome to Moonlight"
        description="Your library is empty. Connect an emulator or import your Steam games to get started."
        action={
          <div className="flex gap-3">
            <Button onClick={() => navigate({ name: "settings", section: "emulators" })}>
              Connect an emulator
            </Button>
            <Button variant="glass" onClick={() => navigate({ name: "settings", section: "libraries" })}>
              Import Steam games
            </Button>
          </div>
        }
      />
    );
  }

  return (
    <div className="h-full overflow-y-auto px-8 pb-12">
      {/* Hero */}
      {hero && (
        <motion.section
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          className="card-shadow group relative mb-8 h-72 cursor-pointer overflow-hidden rounded-panel"
          onClick={() => openDetails(hero.id)}
        >
          {artworkUrl(hero.artwork.background ?? hero.artwork.boxart) ? (
            <img
              src={artworkUrl(hero.artwork.background ?? hero.artwork.boxart)}
              alt=""
              className="h-full w-full object-cover transition-transform duration-700 group-hover:scale-[1.03]"
              draggable={false}
            />
          ) : (
            <div className="h-full w-full bg-gradient-to-br from-accent-soft via-transparent to-transparent" />
          )}
          <div className="absolute inset-0 bg-gradient-to-r from-black/80 via-black/30 to-transparent" />
          <div className="absolute bottom-0 left-0 max-w-lg p-8">
            <p className="mb-2 text-xs font-semibold uppercase tracking-widest text-accent">
              {hero.lastPlayed ? "Continue playing" : "Recently added"}
            </p>
            {artworkUrl(hero.artwork.logo) ? (
              <img src={artworkUrl(hero.artwork.logo)} alt={hero.title} className="mb-3 max-h-20 max-w-xs object-contain" />
            ) : (
              <h2 className="text-shadow-strong mb-2 text-4xl font-bold tracking-tight">{hero.title}</h2>
            )}
            <p className="text-shadow-strong mb-4 text-sm text-ink-dim">
              {hero.lastPlayed
                ? `Last played ${formatRelativeTime(hero.lastPlayed)} · ${formatPlaytime(hero.playtimeSeconds)}`
                : platforms.find((p) => p.id === hero.platformId)?.name}
            </p>
            <Button
              size="lg"
              onClick={(e) => {
                e.stopPropagation();
                onLaunch(hero);
              }}
            >
              <Play className="fill-current" /> Play
            </Button>
          </div>
        </motion.section>
      )}

      {/* Stats strip */}
      <section className="mb-8 grid grid-cols-4 gap-4">
        <StatCard icon={<Gamepad2 />} label="Games" value={String(visible.length)} />
        <StatCard icon={<Layers />} label="Platforms" value={String(sidebarPlatforms.length)} />
        <StatCard icon={<Heart />} label="Favorites" value={String(visible.filter((g) => g.favorite).length)} />
        <StatCard icon={<Clock />} label="Total playtime" value={formatPlaytime(totalPlaytime)} />
      </section>

      <Row
        title="Continue playing"
        icon={<Clock className="size-4" />}
        games={continuePlaying}
        platformsById={new Map(platforms.map((p) => [p.id, p.shortName]))}
        onMore={() => navigate({ name: "recent" })}
        onOpen={(g) => openDetails(g.id)}
        empty="Launch a game and it will appear here."
      />
      <Row
        title="Recently added"
        icon={<Sparkles className="size-4" />}
        games={recentlyAdded}
        platformsById={new Map(platforms.map((p) => [p.id, p.shortName]))}
        onMore={() => navigate({ name: "all" })}
        onOpen={(g) => openDetails(g.id)}
      />
      <Row
        title="Favorites"
        icon={<Heart className="size-4" />}
        games={favorites}
        platformsById={new Map(platforms.map((p) => [p.id, p.shortName]))}
        onMore={() => navigate({ name: "favorites" })}
        onOpen={(g) => openDetails(g.id)}
        empty="Press F on a game (or the heart) to favorite it."
      />

      {/* Platform shortcuts */}
      <section className="mt-2">
        <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold uppercase tracking-wider text-ink-dim">
          <Layers className="size-4" /> Platforms
        </h3>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4">
          {sidebarPlatforms.map((platform) => (
            <button
              key={platform.id}
              className="focusable glass card-shadow flex items-center justify-between rounded-card px-5 py-4 text-left transition-all hover:border-edge-strong hover:bg-glass-strong cursor-pointer"
              onClick={() => navigate({ name: "platform", platformId: platform.id })}
            >
              <div>
                <div className="font-semibold">{platform.name}</div>
                <div className="text-xs text-ink-faint">
                  {platform.count} {platform.count === 1 ? "game" : "games"}
                </div>
              </div>
              <ArrowRight className="size-4 text-ink-faint" />
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}

function StatCard({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="glass card-shadow flex items-center gap-4 rounded-card px-5 py-4">
      <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent [&>svg]:size-5">
        {icon}
      </div>
      <div className="min-w-0">
        <div className="truncate text-lg font-bold tabular-nums">{value}</div>
        <div className="text-xs text-ink-faint">{label}</div>
      </div>
    </div>
  );
}

function Row({
  title,
  icon,
  games,
  platformsById,
  onMore,
  onOpen,
  empty,
}: {
  title: string;
  icon: React.ReactNode;
  games: LibraryEntry[];
  platformsById: Map<string, string>;
  onMore: () => void;
  onOpen: (game: LibraryEntry) => void;
  empty?: string;
}) {
  if (games.length === 0 && !empty) return null;
  return (
    <section className="mb-8">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="flex items-center gap-2 text-sm font-semibold uppercase tracking-wider text-ink-dim">
          {icon} {title}
        </h3>
        {games.length > 0 && (
          <button className="focusable rounded-md text-xs text-ink-faint hover:text-ink cursor-pointer" onClick={onMore}>
            See all →
          </button>
        )}
      </div>
      {games.length === 0 ? (
        <p className="text-sm text-ink-faint">{empty}</p>
      ) : (
        <div className="grid grid-cols-6 gap-4">
          {games.map((game) => (
            <button
              key={game.id}
              className="focusable group overflow-hidden rounded-card glass card-shadow text-left transition-transform hover:-translate-y-1 cursor-pointer"
              onClick={() => onOpen(game)}
            >
              <div className="aspect-[3/4] w-full overflow-hidden bg-[#15151e]">
                {artworkUrl(game.artwork.boxart) ? (
                  <img
                    src={artworkUrl(game.artwork.boxart)}
                    alt=""
                    loading="lazy"
                    className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                    draggable={false}
                  />
                ) : (
                  <div className={cn("flex h-full items-center justify-center p-3 text-center text-xs text-ink-dim")}>
                    {game.title}
                  </div>
                )}
              </div>
              <div className="px-2.5 py-2">
                <div className="truncate text-xs font-semibold">{game.title}</div>
                <div className="truncate text-[10px] text-ink-faint">
                  {platformsById.get(game.platformId) ?? game.platformId}
                </div>
              </div>
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
