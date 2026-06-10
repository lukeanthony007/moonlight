import { create } from "zustand";
import * as api from "@/lib/api";
import type { ScanProgress, ScanReport, ScanScopeInput } from "@/lib/types";

interface ScanState {
  activeScanId: string | null;
  progress: ScanProgress | null;
  lastReport: ScanReport | null;
  /** True once the report banner has been dismissed. */
  reportDismissed: boolean;
  error: string | null;

  start: (scope: ScanScopeInput) => Promise<void>;
  cancel: () => Promise<void>;
  onProgress: (progress: ScanProgress) => void;
  onComplete: (report: ScanReport) => void;
  dismissReport: () => void;
  loadLastReport: () => Promise<void>;
}

export const useScanStore = create<ScanState>((set, get) => ({
  activeScanId: null,
  progress: null,
  lastReport: null,
  reportDismissed: false,
  error: null,

  start: async (scope) => {
    if (get().activeScanId) return;
    set({ error: null, reportDismissed: false });
    try {
      const scanId = await api.startScan(scope);
      set({ activeScanId: scanId, progress: null });
    } catch (e) {
      set({ error: api.errorMessage(e) });
    }
  },

  cancel: async () => {
    const scanId = get().activeScanId;
    if (scanId) await api.cancelScan(scanId);
  },

  onProgress: (progress) => {
    // Adopt scans started elsewhere (e.g. scan-on-startup).
    set({ progress, activeScanId: progress.scanId });
  },

  onComplete: (report) => {
    set({ activeScanId: null, progress: null, lastReport: report, reportDismissed: false });
  },

  dismissReport: () => set({ reportDismissed: true }),

  loadLastReport: async () => {
    try {
      const report = await api.getLastScanReport();
      if (report) set({ lastReport: report, reportDismissed: true });
    } catch {
      // non-fatal
    }
  },
}));
