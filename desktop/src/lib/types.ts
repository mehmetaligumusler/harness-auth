// DTO types mirroring the Rust v2 backend's serde output (camelCase).

export type Theme = "dark" | "light";
export type Language = "tr" | "en";
export type PlanId = "coding-plan" | "start-plan";
export type ProviderId = "zai" | "bigmodel";
export type AccountKind =
  | "zcode_oauth"
  | "z_api"
  | "claude_oauth"
  | "antigravity_oauth"
  | "custom_provider";
export type HarnessId = "claude" | "opencode" | "zcode" | "antigravity";

export interface CustomModelDef {
  id: string;
  name: string;
  contextLimit: number | null;
  outputLimit: number | null;
}

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
  zcodeServiceAccountId: string | null;
  claudeActiveMethod: string;
  claudeActiveSessionId: string | null;
  claudeActiveModel: string | null;
  opencodeActiveSessionId: string | null;
  opencodeActiveMethod: string;
  opencodeActiveModel: string | null;
  zcodeAppActiveAccountId: string | null;
  zcodeAppActiveMethod: string;
  antigravityActiveSessionId: string | null;
}

export interface UnifiedAccountSummary {
  id: string;
  name: string;
  kind: AccountKind;
  provider: string;
  enabled: boolean;
  keyHint: string;
  userHint: string | null;
  baseUrl: string | null;
  package: string | null;
  defaultPlan: PlanId | null;
  defaultModel: string | null;
  models: CustomModelDef[];
  hasJwt: boolean;
  hasSecret: boolean;
  expiresAt: number | null;
  createdAt: number;
  lastUsedAt: number | null;
  supportedHarnesses: string[];
  activeIn: string[];
}

export interface CliInfo {
  found: boolean;
  version: string | null;
  path: string | null;
}

export interface ClaudeLiveState {
  cli: CliInfo;
  detectedMode: string;
  hasCredentialsFile: boolean;
  liveAccountHint: string | null;
  liveExpiresAt: number | null;
  liveBaseUrl: string | null;
  liveModel: string | null;
}

export interface OpenCodeProviderView {
  id: string;
  name: string;
  package: string | null;
  baseUrl: string | null;
  keyHint: string;
  models: CustomModelDef[];
  isManaged: boolean;
}

export interface OpenCodeLiveState {
  cli: CliInfo;
  configPath: string;
  configExists: boolean;
  configSource: "harness_auth" | "custom_provider" | "auth_json" | "none" | string;
  activeModel: string | null;
  baseUrl: string | null;
  providers: OpenCodeProviderView[];
  mcpNames: string[];
  skillsCount: number;
  parseError: string | null;
}

export interface ZCodePersonalRuleView {
  providerId: string;
  providerName: string;
  enabled: boolean;
  baseUrl: string | null;
  keyHint: string;
  models: string[];
}

export interface ZCodeLiveState {
  installed: boolean;
  appPath: string | null;
  v2Exists: boolean;
  zaiMode: string;
  selectedKey: string | null;
  activePersonalName: string | null;
  personalRules: ZCodePersonalRuleView[];
}

export interface AntigravityLiveState {
  cli: CliInfo;
  hasKeyringToken: boolean;
  hasFileToken: boolean;
  activeEmail: string | null;
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
  zcodeAccountCount: number;
  oauthRunning: boolean;
}

export interface Versions {
  app: string;
  proxy: string | null;
}

export interface PathsInfo {
  config: string;
  errorsLog: string;
  dataDir: string;
  vault: string;
}

export interface AppStateDto {
  settings: Settings;
  accounts: UnifiedAccountSummary[];
  proxy: ProxyStatusDto;
  claudeLive: ClaudeLiveState;
  opencodeLive: OpenCodeLiveState;
  zcodeLive: ZCodeLiveState;
  antigravityLive: AntigravityLiveState;
  harnessOauthRunning: boolean;
  harnessOauthTarget: string | null;
  versions: Versions;
  paths: PathsInfo;
}

export interface TestResult {
  ok: boolean;
  message: string;
  latencyMs: number | null;
}

export interface ImportResultDto {
  imported: number;
  details: string[];
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

export interface LogsPage {
  ok: boolean;
  event?: string;
  nextSince?: number;
  lines?: string[];
  error?: string;
}

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
