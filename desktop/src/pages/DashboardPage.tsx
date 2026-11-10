import * as React from "react";
import {
  Bot,
  ExternalLink,
  Layers,
  Play,
  RotateCw,
  Settings2,
  Square,
  UserRound,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
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
import { api } from "@/lib/api";
import { timeAgo } from "@/lib/utils";

function ProxyCard() {
  const t = useT();
  const proxy = useApp((s) => s.state?.proxy);
  const busy = useApp((s) => !!s.busy["proxy"]);
  const start = useApp((s) => s.proxyStart);
  const stop = useApp((s) => s.proxyStop);
  const restart = useApp((s) => s.proxyRestart);

  const running = !!proxy?.processAlive && !!proxy?.serverRunning;
  const processOnly = !!proxy?.processAlive && !proxy?.serverRunning;

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <StatusDot tone={running ? "running" : processOnly ? "warn" : "stopped"} />
          {t("cardProxy")}
        </CardTitle>
        {proxy?.adopted && <Badge variant="secondary">{t("statusAdopted")}</Badge>}
      </CardHeader>
      <CardContent>
        <KeyValue
          label={t("proxyStatusLine")}
          value={
            running
              ? t("statusServerRunning")
              : processOnly
                ? t("statusServerStopped")
                : t("statusProcessDown")
          }
        />
        <KeyValue label={t("proxyEndpoint")} value={proxy?.endpoint ?? t("notAvailable")} mono />
        <KeyValue
          label={t("proxyPort")}
          value={proxy?.serverPort ?? proxy?.configuredPort ?? t("notAvailable")}
        />
        <div className="mt-4 flex flex-wrap gap-2">
          {running ? (
            <Button size="sm" variant="destructive" onClick={stop} disabled={busy}>
              <Square /> {t("proxyStop")}
            </Button>
          ) : (
            <Button size="sm" onClick={start} disabled={busy}>
              <Play /> {t("proxyStart")}
            </Button>
          )}
          <Button
            size="sm"
            variant="outline"
            onClick={restart}
            disabled={busy || !proxy?.processAlive}
          >
            <RotateCw /> {t("proxyRestart")}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function AccountCard() {
  const t = useT();
  const state = useApp((s) => s.state);
  const proxy = state?.proxy;
  const account = state?.accounts.find((a) => a.id === proxy?.activeAccountId);

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <UserRound className="h-4 w-4 text-muted-foreground" />
          {t("cardAccount")}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {account ? (
          <>
            <KeyValue label={t("account")} value={account.name} />
            <KeyValue
              label={t("connectionType")}
              value={
                account.kind === "oauth"
                  ? account.hasJwt
                    ? t("connectionOAuthCoding") + " / " + t("connectionOAuthStart")
                    : t("connectionOAuthCoding")
                  : t("connectionApiKeyStandard")
              }
            />
            <KeyValue
              label={t("provider")}
              value={account.provider === "zai" ? "Z.AI" : t("connectionBigmodel")}
            />
            <KeyValue label={t("statusRunning")} value={proxy?.loggedIn ? "✓" : "—"} />
            <KeyValue label={t("lastUsed")} value={timeAgo(account.lastUsedAt)} />
          </>
        ) : (
          <p className="py-2 text-sm text-muted-foreground">{t("none")}</p>
        )}
      </CardContent>
    </Card>
  );
}

function PlanModelCard() {
  const t = useT();
  const state = useApp((s) => s.state);
  const proxy = state?.proxy;
  const account = state?.accounts.find((a) => a.id === proxy?.activeAccountId);
  const [cfg, setCfg] = React.useState<{ plan: string; auto: boolean; model: string } | null>(null);

  React.useEffect(() => {
    api
      .getConfig()
      .then((c) => setCfg({ plan: c.plan, auto: c.planAutoSwitch, model: c.defaultModel }))
      .catch(() => setCfg(null));
  }, [proxy?.loggedIn]);

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Layers className="h-4 w-4 text-muted-foreground" />
          {t("cardPlan")} / {t("cardModel")}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <KeyValue
          label={t("plan")}
          value={
            cfg?.auto
              ? `${t("autoSwitch")} (${cfg.plan === "coding-plan" ? t("planCoding") : t("planStart")})`
              : cfg?.plan === "coding-plan"
                ? t("planCoding")
                : cfg?.plan === "start-plan"
                  ? t("planStart")
                  : t("notAvailable")
          }
        />
        <KeyValue label={t("model")} value={cfg?.model ?? t("notAvailable")} mono />
        {account && (
          <KeyValue
            label={t("sessionState")}
            value={account.kind === "oauth" ? "OAuth" : "API Key"}
          />
        )}
      </CardContent>
    </Card>
  );
}

export function DashboardPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const initialized = useApp((s) => s.initialized);
  const setPage = useApp((s) => s.setPage);
  const quota = useApp((s) => s.quota);
  const quotaLoading = useApp((s) => s.quotaLoading);
  const quotaError = useApp((s) => s.quotaError);
  const fetchQuota = useApp((s) => s.fetchQuota);
  const logs = useApp((s) => s.logs);
  const pollLogs = useApp((s) => s.pollLogs);
  const resetLogs = useApp((s) => s.resetLogs);
  const errorLogs = useApp((s) => s.errorLogs);
  const fetchErrorLogs = useApp((s) => s.fetchErrorLogs);
  const quickLaunchBusy = React.useRef(false);

  const proxy = state?.proxy;
  const running = !!proxy?.processAlive && !!proxy?.serverRunning;

  // Live log tail while the dashboard is visible.
  React.useEffect(() => {
    if (!running) return;
    resetLogs();
    void pollLogs();
    void fetchErrorLogs(100);
    const id = setInterval(() => void pollLogs(), 2000);
    return () => clearInterval(id);
  }, [running, pollLogs, resetLogs, fetchErrorLogs]);

  React.useEffect(() => {
    if (running) void fetchQuota();
  }, [running, fetchQuota]);

  const quickLaunch = async () => {
    if (quickLaunchBusy.current) return;
    quickLaunchBusy.current = true;
    try {
      await api.claudeLaunch({ mode: "proxy" });
    } catch (e) {
      toast.error(t("errGeneric"), { description: typeof e === "string" ? e : String(e) });
    } finally {
      quickLaunchBusy.current = false;
    }
  };

  if (!initialized || !state) {
    return <div className="py-24 text-center text-muted-foreground">{t("loading")}</div>;
  }

  if (state.accounts.length === 0) {
    return (
      <>
        <PageHeader title={t("dashboardTitle")} subtitle={t("dashboardSubtitle")} />
        <EmptyState
          icon={<Bot className="h-12 w-12" />}
          title={t("welcomeTitle")}
          description={t("welcomeBody")}
          action={
            <Button onClick={() => setPage("accounts")}>
              <UserRound /> {t("welcomeCta")}
            </Button>
          }
        />
      </>
    );
  }

  const recentLines = logs.slice(-12).reverse();
  const recentErrors = errorLogs.slice(-5).reverse();

  return (
    <>
      <PageHeader
        title={t("dashboardTitle")}
        subtitle={t("dashboardSubtitle")}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => setPage("claude")}>
              <Settings2 /> {t("launchMode")}
            </Button>
            <Button size="sm" disabled={!running} onClick={quickLaunch}>
              <Zap /> {t("launchClaude")}
            </Button>
          </>
        }
      />

      <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
        <ProxyCard />
        <AccountCard />
        <PlanModelCard />
      </div>

      <div className="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <QuotaPanel
          quota={quota}
          loading={quotaLoading}
          error={quotaError}
          proxyOff={!running}
          onRefresh={() => void fetchQuota()}
        />

        <div className="space-y-4">
          <Card>
            <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-base">{t("recentRequests")}</CardTitle>
              <button
                type="button"
                onClick={() => setPage("logs")}
                className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
              >
                {t("viewAllLogs")} <ExternalLink className="h-3 w-3" />
              </button>
            </CardHeader>
            <CardContent>
              {recentLines.length === 0 ? (
                <p className="py-4 text-sm text-muted-foreground">{t("noRequestsYet")}</p>
              ) : (
                <div className="max-h-44 space-y-1 overflow-y-auto font-mono text-[11px] leading-relaxed">
                  {recentLines.map((line, i) => (
                    <div key={i} className="truncate text-muted-foreground">
                      {line}
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-base">{t("recentErrors")}</CardTitle>
            </CardHeader>
            <CardContent>
              {recentErrors.length === 0 ? (
                <p className="py-2 text-sm text-muted-foreground">{t("noErrors")}</p>
              ) : (
                <div className="max-h-32 space-y-1.5 overflow-y-auto text-xs">
                  {recentErrors.map((e, i) => (
                    <div key={i} className="flex items-center gap-2">
                      <Badge variant={Number(e.status) >= 500 ? "destructive" : "warning"}>
                        {String(e.status ?? e.kind ?? "?")}
                      </Badge>
                      <span className="truncate text-muted-foreground">
                        {String(e.model ?? "")} {String(e.error ?? "")}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
