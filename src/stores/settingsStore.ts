import { create } from "zustand";
import * as api from "@/lib/api";
import type { AppInfo, ProviderStatus } from "@/lib/types";

interface SettingsState {
  settings: Record<string, unknown>;
  appInfo: AppInfo | null;
  providers: ProviderStatus[];
  loaded: boolean;

  load: () => Promise<void>;
  set: (key: string, value: unknown) => Promise<void>;
  get: <T>(key: string, fallback: T) => T;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: {},
  appInfo: null,
  providers: [],
  loaded: false,

  load: async () => {
    try {
      const [settings, appInfo, providers] = await Promise.all([
        api.getSettings(),
        api.getAppInfo(),
        api.getProviderStatuses(),
      ]);
      set({ settings, appInfo, providers, loaded: true });
    } catch {
      set({ loaded: true });
    }
  },

  set: async (key, value) => {
    set({ settings: { ...get().settings, [key]: value } });
    await api.setSetting(key, value);
    // Provider configuration may change provider statuses.
    if (key.startsWith("providers.")) {
      const providers = await api.getProviderStatuses();
      set({ providers });
    }
  },

  get: <T>(key: string, fallback: T): T => {
    const value = get().settings[key];
    return value === undefined || value === null ? fallback : (value as T);
  },
}));
