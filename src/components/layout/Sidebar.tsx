import { useMemo, useState } from "react";
import {
  ChevronDown,
  Clock,
  FolderHeart,
  Heart,
  Home,
  Layers,
  LibraryBig,
  Moon,
  Plus,
  Settings,
} from "lucide-react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { platformsWithGames, useLibraryStore } from "@/stores/libraryStore";
import { useUiStore, type Route } from "@/stores/uiStore";
import * as api from "@/lib/api";

function routeKey(route: Route): string {
  switch (route.name) {
    case "platform":
      return `platform:${route.platformId}`;
    case "collection":
      return `collection:${route.collectionId}`;
    case "settings":
      return "settings";
    default:
      return route.name;
  }
}

interface NavItemProps {
  label: string;
  icon?: React.ReactNode;
  active: boolean;
  count?: number;
  indent?: boolean;
  onClick: () => void;
}

function NavItem({ label, icon, active, count, indent, onClick }: NavItemProps) {
  return (
    <button
      className={cn(
        "focusable group relative flex w-full items-center gap-3 rounded-xl px-3.5 py-2.5 text-left text-sm transition-colors cursor-pointer",
        indent && "pl-10",
        active ? "text-ink" : "text-ink-dim hover:text-ink hover:bg-glass",
      )}
      onClick={onClick}
      aria-current={active ? "page" : undefined}
    >
      {active && (
        <motion.span
          layoutId="sidebar-active"
          className="absolute inset-0 rounded-xl glass-strong"
          transition={{ type: "spring", stiffness: 480, damping: 38 }}
        />
      )}
      <span className="relative z-10 flex items-center gap-3 min-w-0 flex-1">
        {icon && <span className="shrink-0 [&>svg]:size-4.5">{icon}</span>}
        <span className="truncate font-medium">{label}</span>
      </span>
      {count !== undefined && (
        <span className="relative z-10 text-xs tabular-nums text-ink-faint">{count}</span>
      )}
    </button>
  );
}

export function Sidebar() {
  const route = useUiStore((s) => s.route);
  const navigate = useUiStore((s) => s.navigate);
  const games = useLibraryStore((s) => s.games);
  const platforms = useLibraryStore((s) => s.platforms);
  const collections = useLibraryStore((s) => s.collections);
  const refresh = useLibraryStore((s) => s.refresh);

  const [platformsOpen, setPlatformsOpen] = useState(true);
  const [collectionsOpen, setCollectionsOpen] = useState(true);
  const active = routeKey(route);

  const visiblePlatforms = useMemo(() => platformsWithGames(games, platforms), [games, platforms]);
  const visibleCount = useMemo(() => games.filter((g) => !g.hidden).length, [games]);
  const favoriteCount = useMemo(() => games.filter((g) => g.favorite && !g.hidden).length, [games]);

  async function handleNewCollection() {
    const name = prompt("Collection name");
    if (!name?.trim()) return;
    await api.createCollection(name.trim());
    await refresh();
  }

  return (
    <nav
      aria-label="Library navigation"
      className="glass card-shadow m-3 mr-0 flex w-60 shrink-0 flex-col rounded-panel"
    >
      <div className="flex items-center gap-3 px-5 pb-4 pt-6">
        <div className="flex size-9 items-center justify-center rounded-xl bg-accent-soft">
          <Moon className="size-5 text-accent" />
        </div>
        <span className="text-lg font-bold tracking-tight">Moonlight</span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
        <div className="space-y-0.5">
          <NavItem label="Home" icon={<Home />} active={active === "home"} onClick={() => navigate({ name: "home" })} />
          <NavItem
            label="All Games"
            icon={<LibraryBig />}
            count={visibleCount}
            active={active === "all"}
            onClick={() => navigate({ name: "all" })}
          />
          <NavItem
            label="Recently Played"
            icon={<Clock />}
            active={active === "recent"}
            onClick={() => navigate({ name: "recent" })}
          />
          <NavItem
            label="Favorites"
            icon={<Heart />}
            count={favoriteCount}
            active={active === "favorites"}
            onClick={() => navigate({ name: "favorites" })}
          />
        </div>

        <div className="mt-5">
          <button
            className="focusable flex w-full items-center justify-between rounded-lg px-3.5 py-1.5 text-[11px] font-semibold uppercase tracking-wider text-ink-faint hover:text-ink-dim cursor-pointer"
            onClick={() => setPlatformsOpen((v) => !v)}
            aria-expanded={platformsOpen}
          >
            <span className="flex items-center gap-2">
              <Layers className="size-3.5" /> Platforms
            </span>
            <ChevronDown className={cn("size-3.5 transition-transform", !platformsOpen && "-rotate-90")} />
          </button>
          {platformsOpen && (
            <div className="mt-1 space-y-0.5">
              {visiblePlatforms.length === 0 && (
                <p className="px-3.5 py-2 text-xs text-ink-faint">Import games to see platforms here.</p>
              )}
              {visiblePlatforms.map((platform) => (
                <NavItem
                  key={platform.id}
                  label={platform.name}
                  count={platform.count}
                  indent
                  active={active === `platform:${platform.id}`}
                  onClick={() => navigate({ name: "platform", platformId: platform.id })}
                />
              ))}
            </div>
          )}
        </div>

        <div className="mt-5">
          <button
            className="focusable flex w-full items-center justify-between rounded-lg px-3.5 py-1.5 text-[11px] font-semibold uppercase tracking-wider text-ink-faint hover:text-ink-dim cursor-pointer"
            onClick={() => setCollectionsOpen((v) => !v)}
            aria-expanded={collectionsOpen}
          >
            <span className="flex items-center gap-2">
              <FolderHeart className="size-3.5" /> Collections
            </span>
            <ChevronDown className={cn("size-3.5 transition-transform", !collectionsOpen && "-rotate-90")} />
          </button>
          {collectionsOpen && (
            <div className="mt-1 space-y-0.5">
              {collections.map((collection) => (
                <NavItem
                  key={collection.id}
                  label={collection.name}
                  count={collection.gameIds.length}
                  indent
                  active={active === `collection:${collection.id}`}
                  onClick={() => navigate({ name: "collection", collectionId: collection.id })}
                />
              ))}
              <button
                className="focusable flex w-full items-center gap-2 rounded-xl px-3.5 py-2 pl-10 text-left text-xs text-ink-faint hover:text-ink-dim cursor-pointer"
                onClick={handleNewCollection}
              >
                <Plus className="size-3.5" /> New collection
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="border-t border-edge p-3">
        <NavItem
          label="Settings"
          icon={<Settings />}
          active={active === "settings"}
          onClick={() => navigate({ name: "settings", section: "general" })}
        />
      </div>
    </nav>
  );
}
