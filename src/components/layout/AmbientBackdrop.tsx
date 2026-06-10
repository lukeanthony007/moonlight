import { AnimatePresence, motion } from "framer-motion";
import { artworkUrl } from "@/lib/api";
import type { LibraryEntry } from "@/lib/types";

/** Cinematic ambient backdrop: the focused game's artwork, heavily blurred
 * and dimmed, crossfades behind the whole UI. */
export function AmbientBackdrop({ game }: { game: LibraryEntry | null }) {
  const src = game ? artworkUrl(game.artwork.background ?? game.artwork.boxart ?? game.artwork.screenshot) : undefined;

  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 overflow-hidden bg-base">
      <AnimatePresence mode="popLayout">
        {src && (
          <motion.img
            key={src}
            src={src}
            alt=""
            className="absolute inset-0 h-full w-full scale-125 object-cover blur-[90px] saturate-[1.4]"
            initial={{ opacity: 0 }}
            animate={{ opacity: 0.22 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.9, ease: "easeOut" }}
          />
        )}
      </AnimatePresence>
      {/* Constant vignette to keep text readable over any artwork. */}
      <div className="absolute inset-0 bg-gradient-to-b from-base/60 via-transparent to-base/90" />
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,transparent_0%,rgba(10,10,15,0.55)_100%)]" />
    </div>
  );
}
