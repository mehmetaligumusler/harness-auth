// Central Zustand store for HarnessAuth v2 (4-tab unified architecture).

import { create } from "zustand";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { translate, type TranslationKey } from "@/lib/i18n";
import type {
  AccountKind,
  AppStateDto,
  CustomModelDef,
  HarnessId,
  QuotaSnapshot,
  TestResult,
} from "@/lib/types";

export type PageId = "harnesses" | "accounts" | "settings";

interface AppStore {
  page: PageId;
  setPage: (p: PageId) => void;
  pendingAddKind: AccountKind | null;
  openAddModalFor: (kind: AccountKind) => void;
  clearPendingAddKind: () => void;
  pendingEditAccountId: string | null;
  openEditModalFor: (id: string) => void;
  clearPendingEditAccountId: () => void;

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
  pollLogs: () => Promise<void>;
  errorLogs: Array<Record<string, unknown>>;
  fetchErrorLogs: (limit?: number) => Promise<void>;

  saveSettings: (s: AppStateDto["settings"]) => Promise<void>;

  // Centralized Vault
  addZApiAccount: (p: {
    name: string;
    provider: string;
    apiKey: string;
    defaultModel?: string;
    baseUrl?: string;
  }) => Promise<void>;
  addCustomProvider: (p: {
    name: string;
    providerSlug: string;
    package: string;
    baseUrl: string;
    apiKey: string;
    defaultModel?: string;
    models: CustomModelDef[];
    syncToOpencode: boolean;
  }) => Promise<void>;
  updateUnifiedAccount: (p: {
    id: string;
    name?: string;
    providerSlug?: string;
    package?: string;
    baseUrl?: string;
    apiKey?: string;
    defaultPlan?: string;
    defaultModel?: string;
    models?: CustomModelDef[];
  }) => Promise<void>;
  setAccountEnabled: (id: string, enabled: boolean) => Promise<void>;
  deleteAccount: (id: string) => Promise<void>;
  testAccount: (id: string) => Promise<TestResult>;
  importSystemAccounts: () => Promise<void>;

  // OAuth Flows
  startOAuth: (provider: string, name?: string) => Promise<void>;
  cancelOAuth: () => Promise<void>;
  startClaudeOAuth: (name?: string) => Promise<void>;
  captureClaudeOAuth: (name?: string) => Promise<void>;
  startAgyOAuth: (name?: string) => Promise<void>;
  captureAgyOAuth: (name?: string) => Promise<void>;
  cancelHarnessOAuth: () => Promise<void>;

  // One-click Harness Actions
  activateForHarness: (
    harness: HarnessId,
    accountId: string,
    model?: string,
  ) => Promise<void>;
  disableHarness: (harness: HarnessId) => Promise<void>;
  launchHarness: (harness: HarnessId, folder?: string) => Promise<void>;

  // OpenCode live JSONC actions
  opencodeSetLiveModel: (modelRef: string) => Promise<void>;
  opencodeUpsertLiveProvider: (p: {
    slug: string;
    package: string;
    name: string;
    baseUrl: string;
    apiKey: string;
    models: CustomModelDef[];
    setDefaultModel?: string;
  }) => Promise<void>;
  opencodeRemoveLiveProvider: (slug: string) => Promise<void>;
  opencodeToggleAccount: (
    accountId: string,
    enabled: boolean,
    model?: string,
  ) => Promise<void>;
  opencodeSetDefaultAccount: (
    accountId: string,
    model?: string,
  ) => Promise<void>;

  // ZCode Service
  activateServiceAccount: (id: string) => Promise<void>;
  proxyStart: () => Promise<void>;
  proxyStop: () => Promise<void>;
  proxyRestart: () => Promise<void>;
  proxyKill: () => Promise<void>;
  proxyTest: () => Promise<void>;
}

const errText = (e: unknown) =>
  typeof e === "string"
    ? e
    : e instanceof Error
      ? e.message
      : JSON.stringify(e);

export const useApp = create<AppStore>((set, get) => {
  const withBusy = async <T>(key: string, fn: () => Promise<T>): Promise<T> => {
    set((s) => ({ busy: { ...s.busy, [key]: true } }));
    try {
      return await fn();
    } catch (e) {
      toast.error(errText(e));
      throw e;
    } finally {
      set((s) => ({ busy: { ...s.busy, [key]: false } }));
    }
  };

  return {
    page: "harnesses",
    setPage: (page) => set({ page }),
    pendingAddKind: null,
    openAddModalFor: (kind) => set({ page: "accounts", pendingAddKind: kind }),
    clearPendingAddKind: () => set({ pendingAddKind: null }),
    pendingEditAccountId: null,
    openEditModalFor: (id) => set({ page: "accounts", pendingEditAccountId: id }),
    clearPendingEditAccountId: () => set({ pendingEditAccountId: null }),

    state: null,
    initialized: false,
    initError: null,
    refresh: async () => {
      try {
        const dto = await api.getAppState();
        set({ state: dto, initialized: true, initError: null });
      } catch (e) {
        set({ initError: errText(e), initialized: true });
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
        const quota = await api.fetchQuota();
        set({ quota, quotaLoading: false });
      } catch (e) {
        set({ quotaError: errText(e), quotaLoading: false });
      }
    },

    logs: [],
    logsSince: 0,
    pollLogs: async () => {
      try {
        const page = await api.fetchLogs(get().logsSince);
        if (!page.ok) return;
        const lines = page.lines ?? [];
        set((s) => ({
          logsSince: page.nextSince ?? s.logsSince,
          logs: lines.length > 0 ? [...s.logs, ...lines].slice(-500) : s.logs,
        }));
      } catch {
        /* service down */
      }
    },
    errorLogs: [],
    fetchErrorLogs: async (limit = 150) => {
      try {
        const errorLogs = await api.readErrorLogs(limit);
        set({ errorLogs });
      } catch {
        /* ignore */
      }
    },

    saveSettings: async (newSettings) => {
      await withBusy("settings", async () => {
        const saved = await api.saveSettings(newSettings);
        const cur = get().state;
        if (cur) set({ state: { ...cur, settings: saved } });
        toast.success(translate(saved.language, "saved"));
      });
    },

    addZApiAccount: async (p) => {
      await withBusy("vault", async () => {
        await api.addZApiAccount(p);
        await get().refresh();
        toast.success("Z.AI / BigModel API hesabı eklendi");
      });
    },
    addCustomProvider: async (p) => {
      await withBusy("vault", async () => {
        await api.addCustomProvider(p);
        await get().refresh();
        toast.success("Özel model sağlayıcısı eklendi");
      });
    },
    updateUnifiedAccount: async (p) => {
      await withBusy(`acc:${p.id}`, async () => {
        await api.updateUnifiedAccount(p);
        await get().refresh();
        toast.success("Kayıt güncellendi");
      });
    },
    setAccountEnabled: async (id, enabled) => {
      await withBusy(`acc:${id}`, async () => {
        await api.setAccountEnabled(id, enabled);
        await get().refresh();
        toast.success(enabled ? "Hesap etkinleştirildi" : "Hesap devre dışı bırakıldı");
      });
    },
    deleteAccount: async (id) => {
      await withBusy(`acc:${id}`, async () => {
        await api.deleteAccount(id);
        await get().refresh();
        toast.success("Kayıt silindi");
      });
    },
    testAccount: async (id) => {
      return await withBusy(`test:${id}`, async () => await api.testAccount(id));
    },
    importSystemAccounts: async () => {
      await withBusy("import", async () => {
        const res = await api.importSystemAccounts();
        await get().refresh();
        if (res.imported > 0) {
          toast.success(`${res.imported} kayıt içe aktarıldı`, {
            description: res.details.join(" • "),
          });
        } else {
          toast.info("Sistemde yeni aktarılacak hesap/model bulunamadı");
        }
      });
    },

    startOAuth: async (provider, name) => {
      set({ oauthUrl: null });
      await withBusy("oauth", async () => {
        await api.startOAuth(provider, name);
        await get().refresh();
      });
    },
    cancelOAuth: async () => {
      await api.cancelOAuth();
      set({ oauthUrl: null });
      await get().refresh();
    },
    startClaudeOAuth: async (name) => {
      await withBusy("harnessOauth", async () => {
        await api.startClaudeOAuth(name);
        await get().refresh();
      });
    },
    captureClaudeOAuth: async (name) => {
      await withBusy("captureClaude", async () => {
        await api.captureClaudeOAuth(name);
        await get().refresh();
        toast.success("Claude OAuth oturumu kasaya kaydedildi");
      });
    },
    startAgyOAuth: async (name) => {
      await withBusy("harnessOauth", async () => {
        await api.startAgyOAuth(name);
        await get().refresh();
      });
    },
    captureAgyOAuth: async (name) => {
      await withBusy("captureAgy", async () => {
        await api.captureAgyOAuth(name);
        await get().refresh();
        toast.success("Antigravity CLI oturumu kasaya kaydedildi");
      });
    },
    cancelHarnessOAuth: async () => {
      await api.cancelHarnessOAuth();
      await get().refresh();
    },

    activateForHarness: async (harness, accountId, model) => {
      await withBusy(`harness:${harness}`, async () => {
        await api.activateForHarness({ harness, accountId, model });
        await get().refresh();
        toast.success("Seçilen hesap araçta etkinleştirildi");
      });
    },
    disableHarness: async (harness) => {
      await withBusy(`harness:${harness}`, async () => {
        await api.disableHarness(harness);
        await get().refresh();
        toast.success("Araç bağlantısı devre dışı bırakıldı");
      });
    },
    launchHarness: async (harness, folder) => {
      await withBusy(`launch:${harness}`, async () => {
        if (folder) await api.pushRecentFolder(folder);
        await api.launchHarness(harness, folder);
        toast.success("Başlatıldı");
      });
    },
    opencodeToggleAccount: async (accountId, enabled, model) => {
      await withBusy("harness:opencode", async () => {
        await api.opencodeToggleAccount({ accountId, enabled, model });
        await get().refresh();
        toast.success(
          enabled
            ? "Model/sağlayıcı OpenCode'a eklendi"
            : "Model/sağlayıcı OpenCode'dan kaldırıldı",
        );
      });
    },
    opencodeSetDefaultAccount: async (accountId, model) => {
      await withBusy("harness:opencode", async () => {
        await api.opencodeSetDefaultAccount({ accountId, model });
        await get().refresh();
        toast.success("OpenCode varsayılan modeli güncellendi");
      });
    },

    opencodeSetLiveModel: async (modelRef) => {
      await withBusy("opencodeLive", async () => {
        await api.opencodeSetLiveModel(modelRef);
        await get().refresh();
        toast.success("Varsayılan model güncellendi");
      });
    },
    opencodeUpsertLiveProvider: async (p) => {
      await withBusy("opencodeLive", async () => {
        await api.opencodeUpsertLiveProvider(p);
        await get().refresh();
        toast.success("opencode.jsonc güncellendi");
      });
    },
    opencodeRemoveLiveProvider: async (slug) => {
      await withBusy("opencodeLive", async () => {
        await api.opencodeRemoveLiveProvider(slug);
        await get().refresh();
        toast.success("Sağlayıcı opencode.jsonc içinden kaldırıldı");
      });
    },

    activateServiceAccount: async (id) => {
      await withBusy("service", async () => {
        await api.activateAccount(id);
        await get().refresh();
        void get().fetchQuota();
        toast.success("Servis hesabı değiştirildi");
      });
    },
    proxyStart: async () => {
      await withBusy("service", async () => {
        await api.proxyStart();
        await get().refresh();
        void get().fetchQuota();
      });
    },
    proxyStop: async () => {
      await withBusy("service", async () => {
        await api.proxyStop();
        await get().refresh();
      });
    },
    proxyRestart: async () => {
      await withBusy("service", async () => {
        await api.proxyRestart();
        await get().refresh();
        void get().fetchQuota();
      });
    },
    proxyKill: async () => {
      await withBusy("service", async () => {
        await api.proxyKill();
        await get().refresh();
      });
    },
    proxyTest: async () => {
      await withBusy("proxyTest", async () => {
        const res = await api.proxyTest();
        if (res.ok) toast.success(res.message);
        else toast.error(res.message);
      });
    },
  };
});

export function useT() {
  const lang = useApp((s) => s.state?.settings.language ?? "tr");
  return (key: TranslationKey) => translate(lang, key);
}
