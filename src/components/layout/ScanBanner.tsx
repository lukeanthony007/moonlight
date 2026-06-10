import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, X, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useScanStore } from "@/stores/scanStore";

/** Floating scan status: progress while scanning, result summary after. */
export function ScanBanner() {
  const activeScanId = useScanStore((s) => s.activeScanId);
  const progress = useScanStore((s) => s.progress);
  const lastReport = useScanStore((s) => s.lastReport);
  const reportDismissed = useScanStore((s) => s.reportDismissed);
  const dismissReport = useScanStore((s) => s.dismissReport);
  const cancel = useScanStore((s) => s.cancel);

  const showProgress = activeScanId !== null;
  const showReport = !showProgress && lastReport !== null && !reportDismissed;

  return (
    <div className="pointer-events-none fixed bottom-5 left-1/2 z-50 -translate-x-1/2">
      <AnimatePresence>
        {showProgress && (
          <motion.div
            key="progress"
            role="status"
            aria-live="polite"
            className="glass-strong card-shadow pointer-events-auto flex items-center gap-4 rounded-2xl bg-[#14141f]/95 px-5 py-3.5"
            initial={{ y: 60, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 60, opacity: 0 }}
          >
            <Loader2 className="size-5 shrink-0 animate-spin text-accent" />
            <div className="min-w-0 w-72">
              <div className="text-sm font-medium">
                Scanning library
                {progress?.total ? ` · ${progress.current}/${progress.total}` : "…"}
              </div>
              <div className="truncate text-xs text-ink-dim">{progress?.message ?? "Starting…"}</div>
              {progress?.total ? (
                <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-glass-strong">
                  <div
                    className="h-full rounded-full bg-accent transition-all duration-300"
                    style={{ width: `${Math.min(100, (progress.current / progress.total) * 100)}%` }}
                  />
                </div>
              ) : null}
            </div>
            <Button variant="ghost" size="sm" onClick={() => void cancel()}>
              Cancel
            </Button>
          </motion.div>
        )}

        {showReport && lastReport && (
          <motion.div
            key="report"
            role="status"
            className="glass-strong card-shadow pointer-events-auto flex items-center gap-4 rounded-2xl bg-[#14141f]/95 px-5 py-3.5"
            initial={{ y: 60, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 60, opacity: 0 }}
          >
            {lastReport.errors.length > 0 ? (
              <XCircle className="size-5 shrink-0 text-danger" />
            ) : (
              <CheckCircle2 className="size-5 shrink-0 text-success" />
            )}
            <div className="min-w-0 max-w-md">
              <div className="text-sm font-medium">
                {lastReport.cancelled ? "Scan cancelled" : "Scan complete"}
                <span className="ml-2 text-xs font-normal text-ink-dim">
                  {lastReport.added} added · {lastReport.reconnected} reconnected · {lastReport.missing} missing
                </span>
              </div>
              {lastReport.errors.length > 0 && (
                <div className="mt-0.5 truncate text-xs text-danger" title={lastReport.errors.join("\n")}>
                  {lastReport.errors[0]}
                  {lastReport.errors.length > 1 && ` (+${lastReport.errors.length - 1} more)`}
                </div>
              )}
            </div>
            <button
              aria-label="Dismiss"
              className="focusable rounded-lg p-1 text-ink-faint hover:text-ink cursor-pointer"
              onClick={dismissReport}
            >
              <X className="size-4" />
            </button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
