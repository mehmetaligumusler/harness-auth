// Typed wrappers around Tauri invoke calls.

import { invoke } from "@tauri-apps/api/core";
import type {
  AccountSummary,
  AppStateDto,
  ClaudeMethod,
  ConfigSaveResult,
  HarnessSessionSummary,
  LogsPage,
  ModelsInfo,
  OpenCodeMethod,
  PlanId,
  ProxyConfig,
  ProxyStatusDto,
  QuotaSnapshot,
  Settings,
  TestResult,
} from "./types";

export const api = {
  getAppState: () => invoke<AppStateDto>("get_app_state"),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { newSettings: settings }),

  // ZCode accounts
  addApiKeyAccount: (p: {
    name: string;
    provider: string;
    apiKey: string;
    defaultPlan?: string;
    defaultModel?: string;
    baseUrl?: string;
  }) => invoke<AccountSummary[]>("add_api_key_account", p),
  updateAccount: (p: {
    id: string;
    name?: string;
    defaultPlan?: string;
    defaultModel?: string;
    baseUrl?: string;
  }) => invoke<AccountSummary[]>("update_account", p),
  setAccountEnabled: (id: string, enabled: boolean) =>
    invoke<AccountSummary[]>("set_account_enabled", { id, enabled }),
  deactivateAccount: () => invoke<ProxyStatusDto>("deactivate_account"),
  deleteAccount: (id: string) => invoke<unknown>("delete_account", { id }),
  activateAccount: (id: string) => invoke<ProxyStatusDto>("activate_account", { id }),
  testAccount: (id: string) => invoke<TestResult>("test_account", { id }),

  startOAuth: (provider: string, name?: string) =>
    invoke<void>("start_oauth", { provider, name }),
  cancelOAuth: () => invoke<void>("cancel_oauth"),

  // Proxy engine
  proxyStart: () => invoke<ProxyStatusDto>("proxy_start"),
  proxyStop: () => invoke<ProxyStatusDto>("proxy_stop"),
  proxyRestart: () => invoke<ProxyStatusDto>("proxy_restart"),
  proxyKill: () => invoke<ProxyStatusDto>("proxy_kill"),
  proxyStatus: () => invoke<ProxyStatusDto>("proxy_status"),
  proxyTest: () =>
    invoke<{ ok: boolean; health: boolean; modelsCount: number; message: string }>("proxy_test"),

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

  claudeDetect: () =>
    invoke<{ found: boolean; version: string | null; path: string | null }>("claude_detect"),
  claudeLaunch: (p: { mode: "proxy" | "normal"; model?: string; folder?: string }) =>
    invoke<void>("claude_launch", p),
  pushRecentFolder: (folder: string) => invoke<void>("push_recent_folder", { folder }),
  getVersions: () => invoke<{ app: string; proxy: string | null }>("get_versions"),

  // Multi-harness sessions
  updateSession: (id: string, name: string) =>
    invoke<HarnessSessionSummary[]>("update_session", { id, name }),
  setSessionEnabled: (id: string, enabled: boolean) =>
    invoke<HarnessSessionSummary[]>("set_session_enabled", { id, enabled }),
  deleteSession: (id: string) => invoke<unknown>("delete_session", { id }),
  cancelHarnessOAuth: () => invoke<void>("cancel_harness_oauth"),

  // Claude Code
  claudeCaptureOAuth: (name?: string) =>
    invoke<HarnessSessionSummary>("claude_capture_oauth", { name }),
  claudeStartOAuthLogin: (name?: string) =>
    invoke<void>("claude_start_oauth_login", { name }),
  claudeAddApiSession: (p: {
    name: string;
    providerId: string;
    apiKey: string;
    baseUrl?: string;
    model?: string;
  }) => invoke<HarnessSessionSummary>("claude_add_api_session", p),
  claudeEditSession: (p: {
    id: string;
    name?: string;
    providerId?: string;
    apiKey?: string;
    baseUrl?: string;
    model?: string;
  }) => invoke<HarnessSessionSummary[]>("claude_edit_session", p),
  claudeSetMethod: (p: {
    method: ClaudeMethod;
    targetId?: string;
    model?: string;
    clearOauthCredentials?: boolean;
  }) => invoke<void>("claude_set_method", p),
  claudeLaunchFolder: (folder?: string) =>
    invoke<void>("claude_launch_folder", { folder }),

  // OpenCode
  opencodeCaptureCurrent: (name?: string) =>
    invoke<HarnessSessionSummary>("opencode_capture_current", { name }),
  opencodeAddApiSession: (p: {
    name: string;
    providerId: string;
    apiKey: string;
    baseUrl?: string;
    model?: string;
  }) => invoke<HarnessSessionSummary>("opencode_add_api_session", p),
  opencodeAddZcodeSession: (p: {
    name: string;
    zcodeAccountId: string;
    model?: string;
  }) => invoke<HarnessSessionSummary>("opencode_add_zcode_session", p),
  opencodeStartOAuthLogin: (name?: string) =>
    invoke<void>("opencode_start_oauth_login", { name }),
  opencodeActivateSession: (id: string) =>
    invoke<void>("opencode_activate_session", { id }),
  opencodeSetMethod: (p: {
    method: OpenCodeMethod;
    targetId?: string;
    model?: string;
  }) => invoke<void>("opencode_set_method", p),
  opencodeEditSession: (p: {
    id: string;
    name?: string;
    providerId?: string;
    apiKey?: string;
    baseUrl?: string;
    model?: string;
    zcodeAccountId?: string;
  }) => invoke<HarnessSessionSummary[]>("opencode_edit_session", p),
  opencodeDisable: () => invoke<void>("opencode_disable"),
  opencodeLaunchFolder: (folder?: string) =>
    invoke<void>("opencode_launch_folder", { folder }),

  // Antigravity CLI (agy)
  antigravityCaptureCurrent: (name?: string) =>
    invoke<HarnessSessionSummary>("antigravity_capture_current", { name }),
  antigravityStartOAuthLogin: (name?: string) =>
    invoke<void>("antigravity_start_oauth_login", { name }),
  antigravityActivateSession: (id: string) =>
    invoke<void>("antigravity_activate_session", { id }),
  antigravityDisable: () => invoke<void>("antigravity_disable"),
  antigravityLaunchFolder: (folder?: string) =>
    invoke<void>("antigravity_launch_folder", { folder }),
};
