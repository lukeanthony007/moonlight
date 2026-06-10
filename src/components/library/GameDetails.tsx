import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Calendar,
  Clock,
  EyeOff,
  Eye,
  FolderOpen,
  Gamepad2,
  Heart,
  ImagePlus,
  Pencil,
  Play,
  Star,
  Trash2,
  X,
} from "lucide-react";
import * as api from "@/lib/api";
import { artworkUrl, errorMessage } from "@/lib/api";
import type { LibraryEntry, Platform, PlaySession } from "@/lib/types";
import { cn, formatDate, formatPlaytime, formatRelativeTime } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge, Tooltip } from "@/components/ui/misc";

interface GameDetailsProps {
  game: LibraryEntry;
  platform?: Platform;
  running: boolean;
  onClose: () => void;
  onToggleFavorite: () => void;
  onToggleHidden: () => void;
  onEditMetadata: () => void;
  onEditArtwork: () => void;
  onDelete: () => void;
}

/** Slide-in detail drawer with hero artwork. */
export function GameDetails({
  game,
  platform,
  running,
  onClose,
  onToggleFavorite,
  onToggleHidden,
  onEditMetadata,
  onEditArtwork,
  onDelete,
}: GameDetailsProps) {
  const [sessions, setSessions] = useState<PlaySession[]>([]);
  const [launchError, setLaunchError] = useState<string | null>(null);

  const background = artworkUrl(game.artwork.background ?? game.artwork.screenshot);
  const boxart = artworkUrl(game.artwork.boxart);
  const logo = artworkUrl(game.artwork.logo);
  const install = game.installations.find((i) => i.installed) ?? game.installations[0];

  useEffect(() => {
    setLaunchError(null);
    api.listSessions(game.id, 8).then(setSessions).catch(() => setSessions([]));
  }, [game.id]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  async function handleLaunch() {
    setLaunchError(null);
    try {
      await api.launchGame(game.id);
    } catch (e) {
      setLaunchError(errorMessage(e));
    }
  }

  return (
    <>
      <motion.div
        className="fixed inset-0 z-40 bg-black/55 backdrop-blur-[2px]"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        onClick={onClose}
      />
      <motion.aside
        role="dialog"
        aria-label={`${game.title} details`}
        className="glass-strong card-shadow fixed bottom-3 right-3 top-3 z-50 flex w-[min(560px,92vw)] flex-col overflow-hidden rounded-panel bg-[#101018]/95"
        initial={{ x: 80, opacity: 0 }}
        animate={{ x: 0, opacity: 1 }}
        exit={{ x: 80, opacity: 0 }}
        transition={{ type: "spring", stiffness: 380, damping: 34 }}
      >
        {/* Hero */}
        <div className="relative h-56 shrink-0 overflow-hidden">
          {background ? (
            <img src={background} alt="" className="h-full w-full object-cover" draggable={false} />
          ) : boxart ? (
            <img src={boxart} alt="" className="h-full w-full object-cover blur-2xl scale-125 opacity-60" draggable={false} />
          ) : (
            <div className="h-full w-full bg-gradient-to-br from-accent-soft to-transparent" />
          )}
          <div className="absolute inset-0 bg-gradient-to-t from-[#101018] via-[#101018]/30 to-transparent" />
          <button
            className="focusable absolute right-4 top-4 rounded-full glass p-2 text-ink-dim hover:text-ink cursor-pointer"
            onClick={onClose}
            aria-label="Close details"
          >
            <X className="size-4" />
          </button>
          <div className="absolute bottom-0 left-0 right-0 flex items-end gap-4 p-5">
            {boxart && (
              <img
                src={boxart}
                alt=""
                className="card-shadow h-36 w-auto shrink-0 rounded-xl border border-edge object-cover"
                draggable={false}
              />
            )}
            <div className="min-w-0 pb-1">
              {logo ? (
                <img src={logo} alt={game.title} className="max-h-16 max-w-[260px] object-contain drop-shadow-lg" draggable={false} />
              ) : (
                <h2 className="text-shadow-strong text-2xl font-bold leading-tight tracking-tight">{game.title}</h2>
              )}
              <div className="mt-2 flex flex-wrap items-center gap-1.5">
                <Badge variant="accent">{platform?.name ?? game.platformId}</Badge>
                {game.region && <Badge>{game.region}</Badge>}
                {running && <Badge variant="success">Running</Badge>}
                {install && !install.installed && <Badge variant="danger">File missing</Badge>}
              </div>
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-2.5 px-5 pt-4">
          <Button size="lg" className="flex-1" onClick={handleLaunch} disabled={running}>
            <Play className="fill-current" />
            {running ? "Running…" : "Play"}
          </Button>
          <Tooltip content={game.favorite ? "Remove from favorites" : "Add to favorites"}>
            <Button
              variant="glass"
              size="icon"
              aria-label="Toggle favorite"
              onClick={onToggleFavorite}
              className={cn(game.favorite && "text-danger")}
            >
              <Heart className={cn(game.favorite && "fill-current")} />
            </Button>
          </Tooltip>
          <Tooltip content="Edit metadata">
            <Button variant="glass" size="icon" aria-label="Edit metadata" onClick={onEditMetadata}>
              <Pencil />
            </Button>
          </Tooltip>
          <Tooltip content="Change artwork">
            <Button variant="glass" size="icon" aria-label="Change artwork" onClick={onEditArtwork}>
              <ImagePlus />
            </Button>
          </Tooltip>
        </div>

        {launchError && (
          <div className="mx-5 mt-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
            {launchError}
          </div>
        )}

        {/* Body */}
        <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-6 pt-4">
          {game.description ? (
            <p className="select-text text-sm leading-relaxed text-ink-dim">{game.description}</p>
          ) : (
            <p className="text-sm italic text-ink-faint">
              No description yet. Use “Edit metadata” to add one or fetch it from a provider.
            </p>
          )}

          <dl className="mt-5 grid grid-cols-2 gap-x-6 gap-y-3 text-sm">
            <MetaItem label="Release date" value={formatDate(game.releaseDate)} icon={<Calendar className="size-3.5" />} />
            <MetaItem label="Developer" value={game.developer ?? "Unknown"} />
            <MetaItem label="Publisher" value={game.publisher ?? "Unknown"} />
            <MetaItem label="Genres" value={game.genres.length ? game.genres.join(", ") : "—"} />
            <MetaItem label="Playtime" value={formatPlaytime(game.playtimeSeconds)} icon={<Clock className="size-3.5" />} />
            <MetaItem label="Last played" value={formatRelativeTime(game.lastPlayed)} />
            {game.series && <MetaItem label="Series" value={game.series} />}
            {game.userRating !== null && (
              <MetaItem
                label="Your rating"
                value={`${game.userRating}/5`}
                icon={<Star className="size-3.5 fill-current text-accent" />}
              />
            )}
          </dl>

          {install && (
            <div className="mt-5 rounded-xl glass px-4 py-3">
              <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-ink-faint">
                <FolderOpen className="size-3.5" /> Installation
              </div>
              <div className="mt-1.5 select-text break-all text-xs text-ink-dim">
                {install.sourceType === "steam"
                  ? `Steam App ${install.sourceId}`
                  : (install.path ?? "No file configured")}
              </div>
            </div>
          )}

          {sessions.length > 0 && (
            <div className="mt-5">
              <h3 className="mb-2 flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-ink-faint">
                <Gamepad2 className="size-3.5" /> Recent sessions
              </h3>
              <ul className="space-y-1">
                {sessions.map((s) => (
                  <li key={s.id} className="flex justify-between text-xs text-ink-dim">
                    <span>{formatDate(s.startedAt)}</span>
                    <span>
                      {s.durationSeconds !== null ? formatPlaytime(s.durationSeconds) : "—"}
                      {s.exitStatus && s.exitStatus !== "ok" && (
                        <span className="ml-2 text-ink-faint">({s.exitStatus})</span>
                      )}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="mt-6 flex items-center justify-between border-t border-edge pt-4">
            <Button variant="ghost" size="sm" onClick={onToggleHidden}>
              {game.hidden ? <Eye /> : <EyeOff />}
              {game.hidden ? "Unhide game" : "Hide game"}
            </Button>
            <Button variant="danger" size="sm" onClick={onDelete}>
              <Trash2 /> Remove from library
            </Button>
          </div>
        </div>
      </motion.aside>
    </>
  );
}

function MetaItem({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div>
      <dt className="text-[11px] font-medium uppercase tracking-wider text-ink-faint">{label}</dt>
      <dd className="mt-0.5 flex items-center gap-1.5 text-ink-dim select-text">
        {icon}
        {value}
      </dd>
    </div>
  );
}

/** Wrapper that animates mount/unmount of the drawer. */
export function GameDetailsHost(props: { children: React.ReactNode }) {
  return <AnimatePresence>{props.children}</AnimatePresence>;
}
