import * as React from "react";
import {
  Code2,
  ExternalLink,
  KeyRound,
  Play,
  PowerOff,
  RotateCw,
  Settings2,
  Sparkles,
  Square,
  Terminal,
  UserRound,
} from "lucide-react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui";
import { EmptyState, KeyValue, PageHeader, StatusDot } from "@/components/common";
import { QuotaPanel } from "@/components/QuotaPanel";
import { useApp, useT } from "@/stores/app";
import { formatDateTime } from "@/lib/utils";

function HarnessOverviewCard({
  icon,
  title,
  cliFound,
  cliVersion,
  activeBadge,
  activeVariant,
  rows,
  canDisable,
  onManage,
  onDisable,
  onLaunch,
  busy,
}: {
  icon: React.ReactNode;
  title: string;
  cliFound: boolean;
  cliVersion: string | null;
  activeBadge: string;
  activeVariant: "success" | "default" | "warning" | "secondary";
  rows: Array<{ label: string; value: string; mono?: boolean }>;
  canDisable: boolean;
  onManage: () => void;
  onDisable: () => void;
  onLaunch: () => void;
  busy?: boolean;
}) {
  const t = useT();
  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          {icon}
          {title}
        </CardTitle>
        <div className="flex items-center gap-1.5">
          <Badge variant={cliFound ? "outline" : "warning"}>
            {cliFound ? cliVersion ?? t("cliInstalled") : t("cliNotInstalled")}
          </Badge>
          <Badge variant={activeVariant}>{activeBadge}</Badge>
        </div>
      </CardHeader>
      <CardContent>
        <div className="space-y-0.5 text-sm">
          {rows.map((r) => (
            <KeyValue key={r.label} label={r.label} value={r.value} mono={r.mono} />
          ))}
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button size="sm" variant="outline" onClick={onManage}>
            <Settings2 /> {t("manageBtn")}
          </Button>
          <Button size="sm" disabled={!cliFound || busy} onClick={onLaunch}>
            <Play /> {t("launch")}
          </Button>
          {canDisable && (
            <Button
              size="sm"
              variant="ghost"
              className="text-destructive hover:text-destructive"
              disabled={busy}
              onClick={onDisable}
            >
              <PowerOff /> {t("disableBtn")}
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export function DashboardPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const initialized = useApp((s) => s.initialized);
  const setPage = useApp((s) => s.setPage);
  const busyMap = useApp((s) => s.busy);

  const proxyStart = useApp((s) => s.proxyStart);
  const proxyStop = useApp((s) => s.proxyStop);
  const proxyRestart = useApp((s) => s.proxyRestart);
  const quota = useApp((s) => s.quota);
  const quotaLoading = useApp((s) => s.quotaLoading);
  const quotaError = useApp((s) => s.quotaError);
  const fetchQuota = useApp((s) => s.fetchQuota);
  const logs = useApp((s) => s.logs);
  const pollLogs = useApp((s) => s.pollLogs);
  const resetLogs = useApp((s) => s.resetLogs);

  const claudeSetMethod = useApp((s) => s.claudeSetMethod);
  const claudeLaunchFolder = useApp((s) => s.claudeLaunchFolder);
  const opencodeDisable = useApp((s) => s.opencodeDisable);
  const opencodeLaunchFolder = useApp((s) => s.opencodeLaunchFolder);
  const antigravityDisable = useApp((s) => s.antigravityDisable);
  const antigravityLaunchFolder = useApp((s) => s.antigravityLaunchFolder);

  const proxy = state?.proxy;
  const running = !!proxy?.processAlive && !!proxy?.serverRunning;

  React.useEffect(() => {
    if (!running) return;
    resetLogs();
    void pollLogs();
    void fetchQuota();
    const id = setInterval(() => void pollLogs(), 2500);
    return () => clearInterval(id);
  }, [running, pollLogs, resetLogs, fetchQuota]);

  if (!initialized || !state) {
    return <div className="py-24 text-center text-muted-foreground">{t("loading")}</div>;
  }

  const claudeMode = state.claudeLive.detectedMode;
  const claudeModeBadge =
    claudeMode === "claude_oauth"
      ? t("methodClaudeOAuth")
      : claudeMode === "zcode_oauth"
        ? t("methodZcodeOAuth")
        : claudeMode === "z_api"
          ? t("methodZApi")
          : claudeMode === "custom_api"
            ? "Custom API"
            : t("disabledLabel");

  const activeOcSession = state.sessions.find(
    (s) => s.harness === "opencode" && s.active,
  );
  const ocMode = state.opencodeLive.detectedMode;
  const ocHasLive = ocMode !== "disabled";
  const ocBadge =
    ocMode === "zcode_oauth"
      ? t("methodZcodeOAuth")
      : ocMode === "z_api"
        ? t("methodZApi")
        : activeOcSession
          ? activeOcSession.name
          : ocHasLive
            ? state.opencodeLive.authProviders.join(", ") || t("methodOpencodeProfile")
            : t("disabledLabel");

  const activeAgySession = state.sessions.find(
    (s) => s.harness === "antigravity" && s.active,
  );
  const agyHasLive =
    state.antigravityLive.hasKeyringToken || state.antigravityLive.hasFileToken;

  const activeZcodeAccount = state.accounts.find((a) => a.id === proxy?.activeAccountId);
  const recentLines = logs.slice(-8).reverse();

  return (
    <>
      <PageHeader
        title={t("dashboardTitle")}
        subtitle={t("dashboardSubtitle")}
        actions={
          <Button variant="outline" size="sm" onClick={() => setPage("accounts")}>
            <KeyRound /> {t("navAccounts")} ({state.accounts.length})
          </Button>
        }
      />

      {/* 3 AI Harness Overview Cards */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <HarnessOverviewCard
          icon={<Terminal className="h-4 w-4 text-primary" />}
          title="Claude Code"
          cliFound={state.claudeLive.cli.found}
          cliVersion={state.claudeLive.cli.version}
          activeBadge={claudeModeBadge}
          activeVariant={claudeMode === "disabled" ? "secondary" : "success"}
          canDisable={claudeMode !== "disabled"}
          busy={!!busyMap["claudeMethod"] || !!busyMap["claudeLaunch"]}
          rows={[
            {
              label: t("account"),
              value: state.claudeLive.liveAccountHint ?? t("none"),
              mono: true,
            },
            {
              label: t("model"),
              value: state.claudeLive.liveModel ?? t("useActive"),
              mono: true,
            },
            {
              label: "Endpoint",
              value: state.claudeLive.liveBaseUrl ?? "Anthropic OAuth",
              mono: true,
            },
          ]}
          onManage={() => setPage("claude")}
          onLaunch={() => void claudeLaunchFolder()}
          onDisable={() => void claudeSetMethod({ method: "disabled", clearOauthCredentials: true })}
        />

        <HarnessOverviewCard
          icon={<Code2 className="h-4 w-4 text-primary" />}
          title="OpenCode"
          cliFound={state.opencodeLive.cli.found}
          cliVersion={state.opencodeLive.cli.version}
          activeBadge={ocBadge}
          activeVariant={ocHasLive ? "success" : "secondary"}
          canDisable={ocHasLive}
          busy={!!busyMap["ocDisable"] || !!busyMap["ocLaunch"] || !!busyMap["ocMethod"]}
          rows={[
            {
              label: t("provider"),
              value:
                state.opencodeLive.authProviders.length > 0
                  ? state.opencodeLive.authProviders.join(", ")
                  : t("none"),
              mono: true,
            },
            {
              label: t("model"),
              value: state.opencodeLive.liveModel ?? t("useActive"),
              mono: true,
            },
            {
              label: "Endpoint",
              value: state.opencodeLive.liveBaseUrl ?? "Varsayılan",
              mono: true,
            },
          ]}
          onManage={() => setPage("opencode")}
          onLaunch={() => void opencodeLaunchFolder()}
          onDisable={() => void opencodeDisable()}
        />

        <HarnessOverviewCard
          icon={<Sparkles className="h-4 w-4 text-primary" />}
          title="Antigravity CLI"
          cliFound={state.antigravityLive.cli.found}
          cliVersion={state.antigravityLive.cli.version}
          activeBadge={
            activeAgySession
              ? activeAgySession.name
              : agyHasLive
                ? t("active")
                : t("disabledLabel")
          }
          activeVariant={
            !agyHasLive
              ? "secondary"
              : state.antigravityLive.isExpired
                ? "warning"
                : "success"
          }
          canDisable={agyHasLive}
          busy={!!busyMap["agyDisable"] || !!busyMap["agyLaunch"]}
          rows={[
            {
              label: "Token",
              value: state.antigravityLive.tokenHint ?? t("none"),
              mono: true,
            },
            {
              label: "Keyring",
              value: state.antigravityLive.hasKeyringToken ? "gemini:antigravity ✓" : "—",
              mono: true,
            },
            {
              label: t("quotaExpires"),
              value: state.antigravityLive.expiryDate
                ? formatDateTime(state.antigravityLive.expiryDate)
                : "—",
            },
          ]}
          onManage={() => setPage("antigravity")}
          onLaunch={() => void antigravityLaunchFolder()}
          onDisable={() => void antigravityDisable()}
        />
      </div>

      {/* ZCode Background Bridge & Quotas */}
      {state.accounts.length === 0 && state.sessions.length === 0 ? (
        <div className="mt-6">
          <EmptyState
            icon={<UserRound className="h-10 w-10" />}
            title={t("welcomeTitle")}
            description={t("welcomeBody")}
            action={
              <Button onClick={() => setPage("accounts")}>
                <KeyRound /> {t("welcomeCta")}
              </Button>
            }
          />
        </div>
      ) : (
        <div className="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Card>
            <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="flex items-center gap-2 text-base">
                <StatusDot
                  tone={running ? "running" : proxy?.processAlive ? "warn" : "stopped"}
                />
                ZCode Köprü Servisi
              </CardTitle>
              <button
                type="button"
                onClick={() => setPage("proxy")}
                className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
              >
                {t("navProxy")} <ExternalLink className="h-3 w-3" />
              </button>
            </CardHeader>
            <CardContent className="space-y-3">
              <div>
                <KeyValue
                  label={t("proxyStatusLine")}
                  value={
                    running
                      ? t("statusServerRunning")
                      : proxy?.processAlive
                        ? t("statusServerStopped")
                        : t("statusProcessDown")
                  }
                />
                <KeyValue
                  label={t("cardAccount")}
                  value={activeZcodeAccount?.name ?? t("none")}
                />
                <KeyValue label={t("proxyEndpoint")} value={proxy?.endpoint ?? "—"} mono />
              </div>
              <div className="flex flex-wrap gap-2">
                {running ? (
                  <Button
                    size="sm"
                    variant="destructive"
                    onClick={() => void proxyStop()}
                    disabled={!!busyMap["proxy"]}
                  >
                    <Square /> {t("proxyStop")}
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    onClick={() => void proxyStart()}
                    disabled={!!busyMap["proxy"] || !activeZcodeAccount}
                  >
                    <Play /> {t("proxyStart")}
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void proxyRestart()}
                  disabled={!!busyMap["proxy"] || !proxy?.processAlive}
                >
                  <RotateCw /> {t("proxyRestart")}
                </Button>
              </div>
              {recentLines.length > 0 && (
                <div className="max-h-28 space-y-0.5 overflow-y-auto rounded-md border bg-muted/30 p-2 font-mono text-[11px] text-muted-foreground">
                  {recentLines.map((line, i) => (
                    <div key={i} className="truncate">
                      {line}
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <QuotaPanel
            quota={quota}
            loading={quotaLoading}
            error={quotaError}
            proxyOff={!running}
            onRefresh={() => void fetchQuota()}
          />
        </div>
      )}
    </>
  );
}
