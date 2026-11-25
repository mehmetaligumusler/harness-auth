// Typed wrapper around Tauri `invoke` for HarnessAuth v2.

import { invoke } from "@tauri-apps/api/core";
import type {
  AppStateDto,
  ConfigSaveResult,
  CustomModelDef,
  HarnessId,
  ImportResultDto,
  LogsPage,
  ProxyConfig,
  ProxyStatusDto,
  QuotaSnapshot,
  Settings,
  TestResult,
  UnifiedAccountSummary,
  Versions,
} from "@/lib/types";

export const api = {
  getAppState: () => invoke<AppStateDto>("get_app_state"),
  saveSettings: (newSettings: Settings) =>
    invoke<Settings>("save_settings", { newSettings }),
  resetPanelIdentity: () => invoke<void>("reset_panel_identity"),
  getVersions: () => invoke<Versions>("get_versions"),
  pushRecentFolder: (folder: string) =>
    invoke<void>("push_recent_folder", { folder }),

  // Centralized Unified Vault
  addZApiAccount: (p: {
    name: string;
    provider: string;
    apiKey: string;
    defaultModel?: string;
    baseUrl?: string;
  }) => invoke<UnifiedAccountSummary[]>("add_z_api_account", p),

  addCustomProvider: (p: {
    name: string;
    providerSlug: string;
    package: string;
    baseUrl: string;
    apiKey: string;
    defaultModel?: string;
    models: CustomModelDef[];
    syncToOpencode: boolean;
  }) => invoke<UnifiedAccountSummary[]>("add_custom_provider", p),

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
  }) => invoke<UnifiedAccountSummary[]>("update_unified_account", p),

  setAccountEnabled: (id: string, enabled: boolean) =>
    invoke<UnifiedAccountSummary[]>("set_account_enabled", { id, enabled }),
  deleteAccount: (id: string) =>
    invoke<UnifiedAccountSummary[]>("delete_account", { id }),
  getAccountSecret: (id: string) =>
    invoke<string>("get_account_secret", { id }),
  testAccount: (id: string) =>
    invoke<TestResult>("test_account", { id }),
  importSystemAccounts: () =>
    invoke<ImportResultDto>("import_system_accounts"),

  // OAuth Capture & Browser Login
  startOAuth: (provider: string, name?: string) =>
    invoke<void>("start_oauth", { provider, name }),
  cancelOAuth: () => invoke<void>("cancel_oauth"),
  startClaudeOAuth: (name?: string) =>
    invoke<void>("start_claude_oauth", { name }),
  captureClaudeOAuth: (name?: string) =>
    invoke<UnifiedAccountSummary>("capture_claude_oauth", { name }),
  startAgyOAuth: (name?: string) =>
    invoke<void>("start_agy_oauth", { name }),
  captureAgyOAuth: (name?: string) =>
    invoke<UnifiedAccountSummary>("capture_agy_oauth", { name }),
  cancelHarnessOAuth: () => invoke<void>("cancel_harness_oauth"),

  // One-click Harness Switching & Launch
  activateForHarness: (p: {
    harness: HarnessId;
    accountId: string;
    model?: string;
  }) => invoke<void>("activate_for_harness", p),
  disableHarness: (harness: HarnessId) =>
    invoke<void>("disable_harness", { harness }),
  launchHarness: (harness: HarnessId, folder?: string) =>
    invoke<void>("launch_harness", { harness, folder }),

  // OpenCode live `opencode.jsonc` provider/model controls
  opencodeSetLiveModel: (modelRef: string) =>
    invoke<void>("opencode_set_live_model", { modelRef }),
  opencodeUpsertLiveProvider: (p: {
    slug: string;
    package: string;
    name: string;
    baseUrl: string;
    apiKey: string;
    models: CustomModelDef[];
    setDefaultModel?: string;
  }) => invoke<void>("opencode_upsert_live_provider", p),
  opencodeRemoveLiveProvider: (slug: string) =>
    invoke<void>("opencode_remove_live_provider", { slug }),
  opencodeToggleAccount: (p: {
    accountId: string;
    enabled: boolean;
    model?: string;
  }) => invoke<void>("opencode_toggle_account", p),
  opencodeSetDefaultAccount: (p: {
    accountId: string;
    model?: string;
  }) => invoke<void>("opencode_set_default_account", p),

  // ZCode Background Proxy Service
  activateAccount: (id: string) =>
    invoke<ProxyStatusDto>("activate_account", { id }),
  proxyStart: () => invoke<ProxyStatusDto>("proxy_start"),
  proxyStop: () => invoke<ProxyStatusDto>("proxy_stop"),
  proxyRestart: () => invoke<ProxyStatusDto>("proxy_restart"),
  proxyKill: () => invoke<ProxyStatusDto>("proxy_kill"),
  proxyStatus: () => invoke<ProxyStatusDto>("proxy_status"),
  proxyTest: () =>
    invoke<{ ok: boolean; message: string; modelsCount?: number }>("proxy_test"),
  fetchQuota: () => invoke<QuotaSnapshot>("fetch_quota"),
  fetchLogs: (since: number) => invoke<LogsPage>("fetch_logs", { since }),
  readErrorLogs: (limit = 200) =>
    invoke<Array<Record<string, unknown>>>("read_error_logs", { limit }),
  getConfig: () => invoke<ProxyConfig>("get_config"),
  saveConfig: (updates: {
    port?: number;
    host?: string;
    proxyApiKey?: string;
    provider?: string;
    plan?: string;
    planAutoSwitch?: boolean;
    planPriority?: string[];
    defaultModel?: string;
    models?: string[];
  }) => invoke<ConfigSaveResult>("save_config", { updates }),
  testModel: (model: string) => invoke<TestResult>("test_model", { model }),
};
