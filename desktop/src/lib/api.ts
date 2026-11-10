// Typed wrappers around Tauri invoke calls.

import { invoke } from "@tauri-apps/api/core";
import type {
  AccountSummary,
  AppStateDto,
  ConfigSaveResult,
  LogsPage,
  ModelsInfo,
  PlanId,
  ProxyConfig,
  ProxyStatusDto,
  QuotaSnapshot,
  Settings,
  TestResult,
} from "./types";

export const api = {
  getAppState: () => invoke<AppStateDto>("get_app_state"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { newSettings: settings }),

  addApiKeyAccount: (p: {
    name: string;
    provider: string;
    apiKey: string;
    defaultPlan?: string;
    defaultModel?: string;
  }) => invoke<AccountSummary[]>("add_api_key_account", p),
  updateAccount: (p: {
    id: string;
    name?: string;
    defaultPlan?: string;
    defaultModel?: string;
  }) => invoke<AccountSummary[]>("update_account", p),
  deleteAccount: (id: string) => invoke<unknown>("delete_account", { id }),
  activateAccount: (id: string) => invoke<ProxyStatusDto>("activate_account", { id }),
  testAccount: (id: string) => invoke<TestResult>("test_account", { id }),

  startOAuth: (provider: string, name?: string) =>
    invoke<void>("start_oauth", { provider, name }),
  cancelOAuth: () => invoke<void>("cancel_oauth"),

  proxyStart: () => invoke<ProxyStatusDto>("proxy_start"),
  proxyStop: () => invoke<ProxyStatusDto>("proxy_stop"),
  proxyRestart: () => invoke<ProxyStatusDto>("proxy_restart"),
  proxyKill: () => invoke<ProxyStatusDto>("proxy_kill"),
  proxyStatus: () => invoke<ProxyStatusDto>("proxy_status"),
  proxyTest: () => invoke<{ ok: boolean; health: boolean; modelsCount: number; message: string }>(
    "proxy_test",
  ),

  fetchQuota: () => invoke<QuotaSnapshot>("fetch_quota"),
  fetchLogs: (since: number) => invoke<LogsPage>("fetch_logs", { since }),
  readErrorLogs: (limit: number) =>
    invoke<Array<Record<string, unknown>>>("read_error_logs", { limit }),

  listModels: () => invoke<ModelsInfo>("list_models"),
  saveModelPrefs: (p: {
    defaultModel?: string;
    customModels?: string[];
    recentModel?: string;
  }) => invoke<void>("save_model_prefs", p),
  testModel: (model: string) => invoke<TestResult>("test_model", { model }),

  getConfig: () => invoke<ProxyConfig>("get_config"),
  saveConfig: (updates: {
    port?: number;
    host?: string;
    proxyApiKey?: string;
    provider?: string;
    plan?: PlanId;
    planAutoSwitch?: boolean;
    planPriority?: PlanId[];
    defaultModel?: string;
    models?: string[];
  }) => invoke<ConfigSaveResult>("save_config", { updates }),

  claudeDetect: () => invoke<{ found: boolean; version: string | null; path: string | null }>(
    "claude_detect",
  ),
  claudeLaunch: (p: { mode: "proxy" | "normal"; model?: string; folder?: string }) =>
    invoke<void>("claude_launch", p),
  pushRecentFolder: (folder: string) => invoke<void>("push_recent_folder", { folder }),
  getVersions: () => invoke<{ app: string; proxy: string | null }>("get_versions"),
};
