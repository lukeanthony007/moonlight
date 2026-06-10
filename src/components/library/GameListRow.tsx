import { memo } from "react";
import { Gamepad2, Heart, Play } from "lucide-react";
import { artworkUrl } from "@/lib/api";
import type { LibraryEntry, Platform } from "@/lib/types";
import { cn, formatPlaytime, formatRelativeTime, releaseYear } from "@/lib/utils";

interface GameListRowProps {
  game: LibraryEntry;
  platform?: Platform;
  focused: boolean;
  running: boolean;
  onOpen: () => void;
  onLaunch: () => void;
  onToggleFavorite: () => void;
}

export const GameListRow = memo(function GameListRow({
  game,
  platform,
  focused,
  running,
  onOpen,
  onLaunch,
  onToggleFavorite,
}: GameListRowProps) {
  const boxart = artworkUrl(game.artwork.boxart ?? game.artwork.icon);
  const year = releaseYear(game.releaseDate);

  return (
    <div
      role="button"
      tabIndex={-1}
      data-focused={focused}
      aria-label={game.title}
      className={cn(
        "focusable group mb-1.5 flex h-[58px] cursor-pointer items-center gap-4 rounded-xl px-3 transition-colors",
        "hover:bg-glass border border-transparent hover:border-edge",
        focused && "bg-glass border-edge",
      )}
      onClick={onOpen}
      onDoubleClick={onLaunch}
    >
      <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-[#15151e]">
        {boxart ? (
          <img src={boxart} alt="" className="h-full w-full object-cover" draggable={false} />
        ) : (
          <Gamepad2 className="size-5 text-ink-faint" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-semibold">{game.title}</span>
          {game.favorite && <Heart className="size-3.5 shrink-0 fill-danger text-danger" />}
          {running && (
            <span className="rounded-full bg-success/15 px-2 py-px text-[10px] font-bold uppercase text-success">
              Running
            </span>
          )}
        </div>
        <span className="text-xs text-ink-faint">
          {platform?.name ?? game.platformId}
          {year && ` · ${year}`}
        </span>
      </div>
      <span className="hidden w-28 shrink-0 text-right text-xs text-ink-dim md:block">
        {formatPlaytime(game.playtimeSeconds)}
      </span>
      <span className="hidden w-28 shrink-0 text-right text-xs text-ink-faint lg:block">
        {formatRelativeTime(game.lastPlayed)}
      </span>
      <button
        aria-label={`Launch ${game.title}`}
        className="focusable flex size-8 shrink-0 items-center justify-center rounded-full bg-accent/90 text-[#0d0d18] opacity-0 transition-opacity group-hover:opacity-100 cursor-pointer"
        onClick={(e) => {
          e.stopPropagation();
          onLaunch();
        }}
        tabIndex={-1}
      >
        <Play className="size-3.5 fill-current" />
      </button>
      <button
        aria-label="Toggle favorite"
        className={cn(
          "focusable flex size-8 shrink-0 items-center justify-center rounded-full opacity-0 transition-opacity group-hover:opacity-100 cursor-pointer",
          game.favorite ? "text-danger" : "text-ink-dim",
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
  );
});
