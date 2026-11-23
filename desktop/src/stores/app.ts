// Central app store: state DTO, polling, and every backend action.

import { create } from "zustand";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { translate, type TranslationKey } from "@/lib/i18n";
import type { AppStateDto, ClaudeMethod, OpenCodeMethod, QuotaSnapshot, TestResult } from "@/lib/types";

export type PageId =
  | "dashboard"
  | "claude"
  | "opencode"
  | "antigravity"
  | "accounts"
  | "proxy"
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

  // ZCode Accounts
  addApiKey: (p: {
    name: string;
    provider: string;
    apiKey: string;
    baseUrl?: string;
  }) => Promise<void>;
  updateAccount: (p: {
    id: string;
    name?: string;
    defaultPlan?: string;
    defaultModel?: string;
    baseUrl?: string;
  }) => Promise<void>;
  setAccountEnabled: (id: string, enabled: boolean) => Promise<void>;
  deactivateAccount: () => Promise<void>;
  deleteAccount: (id: string) => Promise<void>;
  activateAccount: (id: string) => Promise<void>;
  testAccount: (id: string) => Promise<TestResult>;

  // ZCode OAuth
  startOAuth: (provider: string, name?: string) => Promise<void>;
  cancelOAuth: () => Promise<void>;

  // Proxy
  proxyStart: () => Promise<void>;
  proxyStop: () => Promise<void>;
  proxyRestart: () => Promise<void>;
  proxyKill: () => Promise<void>;
  proxyTest: () => Promise<void>;

  // Multi-harness Sessions
  updateSession: (id: string, name: string) => Promise<void>;
  setSessionEnabled: (id: string, enabled: boolean) => Promise<void>;
  deleteSession: (id: string) => Promise<void>;
  cancelHarnessOAuth: () => Promise<void>;

  // Claude Code
  claudeCaptureOAuth: (name?: string) => Promise<void>;
  claudeStartOAuthLogin: (name?: string) => Promise<void>;
  claudeAddApiSession: (p: {
    name: string;
    providerId: string;
    apiKey: string;
    baseUrl?: string;
    model?: string;
  }) => Promise<void>;
  claudeEditSession: (p: {
    id: string;
    name?: string;
    providerId?: string;
    apiKey?: string;
    baseUrl?: string;
    model?: string;
  }) => Promise<void>;
  claudeSetMethod: (p: {
    method: ClaudeMethod;
    targetId?: string;
    model?: string;
    clearOauthCredentials?: boolean;
  }) => Promise<void>;
  claudeLaunchFolder: (folder?: string) => Promise<void>;

  // OpenCode
  opencodeCaptureCurrent: (name?: string) => Promise<void>;
  opencodeAddApiSession: (p: {
    name: string;
    providerId: string;
    apiKey: string;
    baseUrl?: string;
    model?: string;
  }) => Promise<void>;
  opencodeAddZcodeSession: (p: {
    name: string;
    zcodeAccountId: string;
    model?: string;
  }) => Promise<void>;
  opencodeStartOAuthLogin: (name?: string) => Promise<void>;
  opencodeActivateSession: (id: string) => Promise<void>;
  opencodeSetMethod: (p: {
    method: OpenCodeMethod;
    targetId?: string;
    model?: string;
  }) => Promise<void>;
  opencodeEditSession: (p: {
    id: string;
    name?: string;
    providerId?: string;
    apiKey?: string;
    baseUrl?: string;
    model?: string;
    zcodeAccountId?: string;
  }) => Promise<void>;
  opencodeDisable: () => Promise<void>;
  opencodeLaunchFolder: (folder?: string) => Promise<void>;

  // Antigravity CLI
  antigravityCaptureCurrent: (name?: string) => Promise<void>;
  antigravityStartOAuthLogin: (name?: string) => Promise<void>;
  antigravityActivateSession: (id: string) => Promise<void>;
  antigravityDisable: () => Promise<void>;
  antigravityLaunchFolder: (folder?: string) => Promise<void>;
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
  setAccountEnabled: async (id, enabled) => {
    await run(`enAcc:${id}`, set, get, () => api.setAccountEnabled(id, enabled), t0("saved"));
  },
  deactivateAccount: async () => {
    await run("deactAcc", set, get, () => api.deactivateAccount(), t0("saved"));
  },
  deleteAccount: async (id) => {
    await run(`delAcc:${id}`, set, get, () => api.deleteAccount(id), t0("saved"));
  },
  activateAccount: async (id) => {
    await run(`actAcc:${id}`, set, get, () => api.activateAccount(id), t0("saved"));
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

  updateSession: async (id, name) => {
    await run(`updSess:${id}`, set, get, () => api.updateSession(id, name), t0("saved"));
  },
  setSessionEnabled: async (id, enabled) => {
    await run(`enSess:${id}`, set, get, () => api.setSessionEnabled(id, enabled), t0("saved"));
  },
  deleteSession: async (id) => {
    await run(`delSess:${id}`, set, get, () => api.deleteSession(id), t0("saved"));
  },
  cancelHarnessOAuth: async () => {
    await api.cancelHarnessOAuth();
    await get().refresh();
  },

  claudeCaptureOAuth: async (name) => {
    await run("claudeCapture", set, get, () => api.claudeCaptureOAuth(name), t0("sessionCaptured"));
  },
  claudeStartOAuthLogin: async (name) => {
    await run("claudeOAuth", set, get, () => api.claudeStartOAuthLogin(name));
  },
  claudeAddApiSession: async (p) => {
    await run("claudeAddApi", set, get, () => api.claudeAddApiSession(p), t0("saved"));
  },
  claudeEditSession: async (p) => {
    await run(`claudeEdit:${p.id}`, set, get, () => api.claudeEditSession(p), t0("saved"));
  },
  claudeSetMethod: async (p) => {
    await run("claudeMethod", set, get, () => api.claudeSetMethod(p), t0("saved"));
  },
  claudeLaunchFolder: async (folder) => {
    await run("claudeLaunch", set, get, () => api.claudeLaunchFolder(folder));
  },

  opencodeCaptureCurrent: async (name) => {
    await run("ocCapture", set, get, () => api.opencodeCaptureCurrent(name), t0("sessionCaptured"));
  },
  opencodeAddApiSession: async (p) => {
    await run("ocAddApi", set, get, () => api.opencodeAddApiSession(p), t0("saved"));
  },
  opencodeAddZcodeSession: async (p) => {
    await run("ocAddZcode", set, get, () => api.opencodeAddZcodeSession(p), t0("saved"));
  },
  opencodeStartOAuthLogin: async (name) => {
    await run("ocOAuth", set, get, () => api.opencodeStartOAuthLogin(name));
  },
  opencodeActivateSession: async (id) => {
    await run(`ocAct:${id}`, set, get, () => api.opencodeActivateSession(id), t0("saved"));
  },
  opencodeSetMethod: async (p) => {
    await run("ocMethod", set, get, () => api.opencodeSetMethod(p), t0("saved"));
  },
  opencodeEditSession: async (p) => {
    await run(`ocEdit:${p.id}`, set, get, () => api.opencodeEditSession(p), t0("saved"));
  },
  opencodeDisable: async () => {
    await run("ocDisable", set, get, () => api.opencodeDisable(), t0("harnessDisabledOk"));
  },
  opencodeLaunchFolder: async (folder) => {
    await run("ocLaunch", set, get, () => api.opencodeLaunchFolder(folder));
  },

  antigravityCaptureCurrent: async (name) => {
    await run(
      "agyCapture",
      set,
      get,
      () => api.antigravityCaptureCurrent(name),
      t0("sessionCaptured"),
    );
  },
  antigravityStartOAuthLogin: async (name) => {
    await run("agyOAuth", set, get, () => api.antigravityStartOAuthLogin(name));
  },
  antigravityActivateSession: async (id) => {
    await run(`agyAct:${id}`, set, get, () => api.antigravityActivateSession(id), t0("saved"));
  },
  antigravityDisable: async () => {
    await run("agyDisable", set, get, () => api.antigravityDisable(), t0("harnessDisabledOk"));
  },
  antigravityLaunchFolder: async (folder) => {
    await run("agyLaunch", set, get, () => api.antigravityLaunchFolder(folder));
  },
}));

/** Translation hook bound to the current UI language. */
export function useT() {
  const lang = useApp((s) => s.state?.settings.language ?? "tr");
  return (key: TranslationKey) => translate(lang, key);
}
