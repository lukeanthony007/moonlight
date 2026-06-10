import { memo, useState } from "react";
import { motion } from "framer-motion";
import { Gamepad2, Heart, Play } from "lucide-react";
import { artworkUrl } from "@/lib/api";
import type { LibraryEntry, Platform } from "@/lib/types";
import { cn, formatRelativeTime, releaseYear } from "@/lib/utils";

interface GameCardProps {
  game: LibraryEntry;
  platform?: Platform;
  focused: boolean;
  running: boolean;
  onOpen: () => void;
  onLaunch: () => void;
  onToggleFavorite: () => void;
}

export const GameCard = memo(function GameCard({
  game,
  platform,
  focused,
  running,
  onOpen,
  onLaunch,
  onToggleFavorite,
}: GameCardProps) {
  const [imageLoaded, setImageLoaded] = useState(false);
  const boxart = artworkUrl(game.artwork.boxart);
  const year = releaseYear(game.releaseDate);
  const installed = game.installations.some((i) => i.installed);

  return (
    <motion.div
      role="button"
      tabIndex={-1}
      aria-label={`${game.title} — ${platform?.name ?? game.platformId}`}
      data-focused={focused}
      className={cn(
        "focusable group relative flex w-full cursor-pointer flex-col overflow-hidden rounded-card glass card-shadow",
        "transition-colors duration-200 hover:border-edge-strong",
      )}
      whileHover={{ y: -4, scale: 1.015 }}
      transition={{ type: "spring", stiffness: 400, damping: 28 }}
      onClick={onOpen}
      onDoubleClick={onLaunch}
    >
      <div className="relative aspect-[3/4] w-full overflow-hidden bg-[#15151e]">
        {boxart ? (
          <img
            src={boxart}
            alt=""
            loading="lazy"
            draggable={false}
            onLoad={() => setImageLoaded(true)}
            className={cn(
              "h-full w-full object-cover transition-all duration-500",
              imageLoaded ? "opacity-100 blur-0" : "opacity-0 blur-md",
              "group-hover:scale-[1.04]",
            )}
          />
        ) : (
          <div className="flex h-full w-full flex-col items-center justify-center gap-3 p-4 text-center">
            <Gamepad2 className="size-10 text-ink-faint" />
            <span className="text-sm font-medium leading-snug text-ink-dim line-clamp-3">{game.title}</span>
          </div>
        )}

        {/* Hover overlay with quick launch */}
        <div
          className={cn(
            "absolute inset-0 flex items-end justify-between bg-gradient-to-t from-black/80 via-transparent to-transparent p-3",
            "opacity-0 transition-opacity duration-200 group-hover:opacity-100",
            focused && "opacity-100",
          )}
        >
          <button
            aria-label={`Launch ${game.title}`}
            className="focusable flex size-10 items-center justify-center rounded-full bg-accent text-[#0d0d18] shadow-lg transition-transform hover:scale-110 cursor-pointer"
            onClick={(e) => {
              e.stopPropagation();
              onLaunch();
            }}
            tabIndex={-1}
          >
            <Play className="size-4.5 fill-current" />
          </button>
          <button
            aria-label={game.favorite ? "Remove from favorites" : "Add to favorites"}
            className={cn(
              "focusable flex size-9 items-center justify-center rounded-full glass transition-all hover:scale-110 cursor-pointer",
              game.favorite ? "text-danger" : "text-ink-dim hover:text-ink",
            )}
            onClick={(e) => {
              e.stopPropagation();
              onToggleFavorite();
            }}
            tabIndex={-1}
          >
            <Heart className={cn("size-4", game.favorite && "fill-current")} />
          </button>
        </div>

        {/* Status chips */}
        <div className="absolute left-2.5 top-2.5 flex flex-col items-start gap-1.5">
          {running && (
            <span className="rounded-full bg-success/90 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide text-[#0d2616] shadow">
              Running
            </span>
          )}
          {!installed && (
            <span className="rounded-full bg-black/60 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-ink-dim backdrop-blur">
              Missing
            </span>
          )}
        </div>
        {game.favorite && !focused && (
          <Heart className="absolute right-2.5 top-2.5 size-4 fill-danger text-danger drop-shadow group-hover:opacity-0 transition-opacity" />
        )}
      </div>

      <div className="flex flex-col gap-0.5 px-3.5 py-3">
        <span className="truncate text-[13px] font-semibold leading-tight text-ink" title={game.title}>
          {game.title}
        </span>
        <span className="truncate text-[11px] text-ink-faint">
          {platform?.shortName ?? game.platformId}
          {year && ` · ${year}`}
          {game.lastPlayed && ` · ${formatRelativeTime(game.lastPlayed)}`}
        </span>
      </div>
    </motion.div>
  );
});
