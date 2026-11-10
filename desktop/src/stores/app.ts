// Central app store: state DTO, polling, and every backend action.

import { create } from "zustand";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { translate, type TranslationKey } from "@/lib/i18n";
import type { AppStateDto, QuotaSnapshot, TestResult } from "@/lib/types";

export type PageId =
  | "dashboard"
  | "accounts"
  | "plans"
  | "models"
  | "claude"
  | "proxy"
  | "logs"
  | "settings";

interface AppStore {
  page: PageId;
  setPage: (p: PageId) => void;

  state: AppStateDto | null;
  initialized: boolean;
  initError: string | null;
  refresh: () => Promise<void>;

  busy: Record<string, boolean>;
  isBusy: (k: string) => boolean;

  oauthUrl: string | null;

  quota: QuotaSnapshot | null;
  quotaLoading: boolean;
  quotaError: string | null;
  fetchQuota: () => Promise<void>;

  logs: string[];
  logsSince: number;
  logsFollow: boolean;
  setLogsFollow: (v: boolean) => void;
  pollLogs: () => Promise<void>;
  resetLogs: () => void;

  errorLogs: Array<Record<string, unknown>>;
  fetchErrorLogs: (limit?: number) => Promise<void>;

  // Settings
  saveSettings: (s: AppStateDto["settings"]) => Promise<void>;

  // Accounts
  addApiKey: (p: { name: string; provider: string; apiKey: string }) => Promise<void>;
  updateAccount: (p: { id: string; name?: string; defaultPlan?: string; defaultModel?: string }) => Promise<void>;
  deleteAccount: (id: string) => Promise<void>;
  activateAccount: (id: string) => Promise<void>;
  testAccount: (id: string) => Promise<TestResult>;

  // OAuth
  startOAuth: (provider: string, name?: string) => Promise<void>;
  cancelOAuth: () => Promise<void>;

  // Proxy
  proxyStart: () => Promise<void>;
  proxyStop: () => Promise<void>;
  proxyRestart: () => Promise<void>;
  proxyKill: () => Promise<void>;
  proxyTest: () => Promise<void>;
}

async function run<T>(
  busyKey: string,
  set: (p: Partial<AppStore>) => void,
  get: () => AppStore,
  fn: () => Promise<T>,
  successMsg?: string,
): Promise<T | undefined> {
  if (get().busy[busyKey]) return undefined;
  set({ busy: { ...get().busy, [busyKey]: true } });
  try {
    const result = await fn();
    await get().refresh();
    if (successMsg) toast.success(successMsg);
    return result;
  } catch (e) {
    const msg = typeof e === "string" ? e : String(e);
    toast.error(translate(lang0(), "errGeneric"), { description: msg });
    return undefined;
  } finally {
    set({ busy: { ...get().busy, [busyKey]: false } });
  }
}

function lang0(): string {
  return useApp.getState().state?.settings.language ?? "tr";
}

function t0(key: TranslationKey): string {
  return translate(lang0(), key);
}

export const useApp = create<AppStore>((set, get) => ({
  page: "dashboard",
  setPage: (p) => set({ page: p }),

  state: null,
  initialized: false,
  initError: null,
  refresh: async () => {
    try {
      const s = await api.getAppState();
      set({ state: s, initialized: true, initError: null });
    } catch (e) {
      const first = !get().initialized;
      set({
        initialized: true,
        initError: typeof e === "string" ? e : String(e),
      });
      if (first) console.error("get_app_state failed", e);
    }
  },

  busy: {},
  isBusy: (k) => !!get().busy[k],

  oauthUrl: null,

  quota: null,
  quotaLoading: false,
  quotaError: null,
  fetchQuota: async () => {
    set({ quotaLoading: true, quotaError: null });
    try {
      const q = await api.fetchQuota();
      set({ quota: q, quotaLoading: false });
    } catch (e) {
      set({ quotaLoading: false, quotaError: typeof e === "string" ? e : String(e) });
    }
  },

  logs: [],
  logsSince: 0,
  logsFollow: true,
  setLogsFollow: (v) => set({ logsFollow: v }),
  pollLogs: async () => {
    try {
      const page = await api.fetchLogs(get().logsSince);
      if (page.ok && page.event === "logs" && page.lines && page.lines.length > 0) {
        const merged = [...get().logs, ...page.lines];
        set({
          logs: merged.length > 400 ? merged.slice(merged.length - 400) : merged,
          logsSince: page.nextSince ?? get().logsSince,
        });
      } else if (page.ok && page.event === "logs") {
        set({ logsSince: page.nextSince ?? get().logsSince });
      }
    } catch {
      // proxy down — keep what we have
    }
  },
  resetLogs: () => set({ logs: [], logsSince: 0 }),

  errorLogs: [],
  fetchErrorLogs: async (limit = 300) => {
    try {
      const rows = await api.readErrorLogs(limit);
      set({ errorLogs: rows });
    } catch {
      set({ errorLogs: [] });
    }
  },

  saveSettings: async (s) => {
    await run("settings", set, get, () => api.saveSettings(s), t0("saved"));
  },

  addApiKey: async (p) => {
    await run("addKey", set, get, () => api.addApiKeyAccount(p), t0("saved"));
  },
  updateAccount: async (p) => {
    await run("updAcc", set, get, () => api.updateAccount(p), t0("saved"));
  },
  deleteAccount: async (id) => {
    await run(`delAcc:${id}`, set, get, () => api.deleteAccount(id), t0("saved"));
  },
  activateAccount: async (id) => {
    await run(`actAcc:${id}`, set, get, () => api.activateAccount(id));
    await get().refresh();
  },
  testAccount: async (id) => {
    return (await run(`testAcc:${id}`, set, get, () => api.testAccount(id))) as TestResult;
  },

  startOAuth: async (provider, name) => {
    set({ oauthUrl: null });
    await run("oauth", set, get, async () => {
      await api.startOAuth(provider, name);
    });
  },
  cancelOAuth: async () => {
    await api.cancelOAuth();
    set({ oauthUrl: null });
    await get().refresh();
  },

  proxyStart: async () => {
    await run("proxy", set, get, () => api.proxyStart(), t0("statusRunning"));
  },
  proxyStop: async () => {
    await run("proxy", set, get, () => api.proxyStop(), t0("statusStopped"));
  },
  proxyRestart: async () => {
    await run("proxy", set, get, () => api.proxyRestart(), t0("saved"));
  },
  proxyKill: async () => {
    await run("proxy", set, get, () => api.proxyKill(), t0("statusProcessDown"));
  },
  proxyTest: async () => {
    await run("proxyTest", set, get, () => api.proxyTest());
  },
}));

/** Translation hook bound to the current UI language. */
export function useT() {
  const lang = useApp((s) => s.state?.settings.language ?? "tr");
  return (key: TranslationKey) => translate(lang, key);
}
