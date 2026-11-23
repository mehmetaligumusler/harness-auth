// DTO types mirroring the Rust backend's serde output (camelCase).

export type Theme = "dark" | "light";
export type Language = "tr" | "en";
export type PlanId = "coding-plan" | "start-plan";
export type ProviderId = "zai" | "bigmodel";
export type ClaudeMethod = "claude_oauth" | "zcode_oauth" | "z_api" | "custom_api" | "disabled";
export type OpenCodeMethod = "profile" | "zcode_oauth" | "z_api" | "disabled";
export type HarnessId = "claude" | "opencode" | "antigravity";

export interface Settings {
  theme: Theme;
  language: Language;
  panelPort: number;
  keepProxyOnExit: boolean;
  proxyAutostart: boolean;
  autostartWindows: boolean;
  startMinimized: boolean;
  closeToTray: boolean;
  defaultProvider: ProviderId;
  defaultPlan: PlanId;
  defaultModel: string;
  claudeDefaultFolder: string | null;
  recentFolders: string[];
  recentModels: string[];
  logLevel: string;
  updateCheck: boolean;
  claudeActiveMethod: ClaudeMethod;
  claudeActiveSessionId: string | null;
  claudeActiveModel: string | null;
  opencodeActiveSessionId: string | null;
  opencodeActiveMethod: OpenCodeMethod;
  opencodeActiveModel: string | null;
  antigravityActiveSessionId: string | null;
}

export interface AccountSummary {
  id: string;
  name: string;
  kind: "oauth" | "api-key";
  provider: ProviderId;
  keyHint: string;
  userHint: string | null;
  hasJwt: boolean;
  hasSecret: boolean;
  createdAt: number;
  lastUsedAt: number | null;
  defaultPlan: PlanId | null;
  defaultModel: string | null;
  enabled: boolean;
  baseUrl: string | null;
  active: boolean;
}

export interface CliInfo {
  found: boolean;
  version: string | null;
  path: string | null;
}

export interface HarnessSessionSummary {
  id: string;
  harness: HarnessId;
  kind:
    | "claude_oauth"
    | "claude_api"
    | "opencode_snapshot"
    | "opencode_api"
    | "opencode_zcode_proxy"
    | "antigravity_oauth";
  name: string;
  enabled: boolean;
  providerHint: string | null;
  accountHint: string | null;
  expiresAt: number | null;
  meta: Record<string, unknown>;
  createdAt: number;
  lastUsedAt: number | null;
  active: boolean;
}

export interface ClaudeLiveState {
  cli: CliInfo;
  detectedMode: "claude_oauth" | "zcode_oauth" | "z_api" | "custom_api" | "disabled";
  hasCredentialsFile: boolean;
  liveAccountHint: string | null;
  liveExpiresAt: number | null;
  liveBaseUrl: string | null;
  liveModel: string | null;
}

export interface OpenCodeLiveState {
  cli: CliInfo;
  authPath: string;
  configPath: string;
  hasAuthFile: boolean;
  authProviders: string[];
  configProviders: string[];
  hasZcodeProvider: boolean;
  detectedMode: OpenCodeMethod;
  liveBaseUrl: string | null;
  liveModel: string | null;
}

export interface AntigravityLiveState {
  cli: CliInfo;
  hasKeyringToken: boolean;
  hasFileToken: boolean;
  tokenHint: string | null;
  expiryDate: number | null;
  isExpired: boolean;
  scope: string | null;
}

export interface ProxyStatus {
  processAlive: boolean;
  adopted: boolean;
  panelPort: number;
  serverRunning: boolean;
  serverPort: number | null;
  provider: ProviderId | null;
  plan: PlanId | null;
  loggedIn: boolean;
  configuredPort: number;
  endpoint: string;
}

export interface ProxyStatusDto extends ProxyStatus {
  activeAccountId: string | null;
  activeAccountName: string | null;
  accountCount: number;
  oauthRunning: boolean;
}

export interface ClaudeInfo {
  found: boolean;
  version: string | null;
  path: string | null;
}

export interface Versions {
  app: string;
  proxy: string | null;
}

export interface PathsInfo {
  config: string;
  errorsLog: string;
  dataDir: string;
}

export interface AppStateDto {
  settings: Settings;
  accounts: AccountSummary[];
  sessions: HarnessSessionSummary[];
  proxy: ProxyStatusDto;
  claude: ClaudeInfo;
  claudeLive: ClaudeLiveState;
  opencodeLive: OpenCodeLiveState;
  antigravityLive: AntigravityLiveState;
  harnessOauthRunning: boolean;
  harnessOauthTarget: HarnessId | null;
  versions: Versions;
  paths: PathsInfo;
}

export interface TestResult {
  ok: boolean;
  message: string;
  latencyMs: number | null;
}

export interface ModelsInfo {
  available: boolean;
  endpointModels: string[];
  configModels: string[];
  defaultModel: string;
}

export interface ProxyConfig {
  port: number;
  host: string;
  proxyApiKey: string | null;
  provider: ProviderId;
  plan: PlanId;
  planAutoSwitch: boolean;
  planPriority: PlanId[];
  planSwitchRules: unknown;
  planPollIntervalSec: number;
  defaultModel: string;
  models: string[];
}

export interface ConfigSaveResult {
  restarted: boolean;
  config: ProxyConfig;
}

// --- Quota snapshot (proxy /quota shape) ---

export interface QuotaBalance {
  showName: string;
  remainingUnits: number;
  totalUnits: number;
  usedUnits: number;
  unitType?: string;
  expiresAt?: string;
}

export interface QuotaLimit {
  type: string;
  unit?: string;
  total?: number;
  used?: number;
  remaining?: number;
  percentage?: number;
  nextResetTime?: string;
}

export interface QuotaSnapshot {
  provider: ProviderId;
  serverTime?: string;
  jwt?: { ageHours?: number; issuedAt?: string };
  balances?: QuotaBalance[];
  claimablePlans?: string[];
  codingPlan?: { level?: string; limits: QuotaLimit[] };
  errors?: string[];
}

// --- Live logs (panel getLogs) ---

export interface LogsPage {
  ok: boolean;
  event?: string;
  nextSince?: number;
  lines?: string[];
  error?: string;
}

// --- OAuth events ---

export interface OAuthDone {
  ok: boolean;
  provider: string;
  accountId: string | null;
  error: string | null;
}

export interface HarnessOAuthDone {
  ok: boolean;
  harness: HarnessId;
  sessionId: string | null;
  error: string | null;
}
