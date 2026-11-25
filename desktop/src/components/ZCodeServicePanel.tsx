import * as React from "react";
import { FlaskConical, Play, RotateCw, Skull, Square } from "lucide-react";
import { toast } from "sonner";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Input,
  Label,
  Select,
  Switch,
} from "@/components/ui";
import { CopyField, StatusDot } from "@/components/common";
import { QuotaPanel } from "@/components/QuotaPanel";
import { api } from "@/lib/api";
import type { PlanId, ProviderId, ProxyConfig, Settings as SettingsType } from "@/lib/types";
import { useApp, useT } from "@/stores/app";

export function ZCodeServicePanel({
  draft,
  onDraftChange,
}: {
  draft: SettingsType;
  onDraftChange: <K extends keyof SettingsType>(key: K, value: SettingsType[K]) => void;
}) {
  const t = useT();
  const state = useApp((s) => s.state);
  const quota = useApp((s) => s.quota);
  const quotaLoading = useApp((s) => s.quotaLoading);
  const quotaError = useApp((s) => s.quotaError);
  const fetchQuota = useApp((s) => s.fetchQuota);
  const logs = useApp((s) => s.logs);
  const pollLogs = useApp((s) => s.pollLogs);
  const errorLogs = useApp((s) => s.errorLogs);
  const fetchErrorLogs = useApp((s) => s.fetchErrorLogs);

  const activateServiceAccount = useApp((s) => s.activateServiceAccount);
  const proxyStart = useApp((s) => s.proxyStart);
  const proxyStop = useApp((s) => s.proxyStop);
  const proxyRestart = useApp((s) => s.proxyRestart);
  const proxyKill = useApp((s) => s.proxyKill);
  const proxyTest = useApp((s) => s.proxyTest);
  const busy = useApp((s) => s.busy);

  const [cfg, setCfg] = React.useState<ProxyConfig | null>(null);
  const [savingCfg, setSavingCfg] = React.useState(false);

  const running = !!state?.proxy.processAlive && !!state?.proxy.serverRunning;

  React.useEffect(() => {
    void api.getConfig().then(setCfg).catch(() => {});
  }, [state?.proxy.activeAccountId]);

  React.useEffect(() => {
    if (!running) return;
    void fetchQuota();
    void pollLogs();
    void fetchErrorLogs(50);
    const id = setInterval(() => {
      void pollLogs();
    }, 3000);
    return () => clearInterval(id);
  }, [running, fetchQuota, pollLogs, fetchErrorLogs]);

  if (!state) return null;

  const serviceAccounts = state.accounts.filter(
    (a) => a.enabled && a.kind === "zcode_oauth",
  );
  const activeId = state.proxy.activeAccountId ?? serviceAccounts[0]?.id ?? "";

  const savePlanConfig = async () => {
    if (!cfg) return;
    setSavingCfg(true);
    try {
      const res = await api.saveConfig({
        port: cfg.port,
        plan: cfg.plan,
        planAutoSwitch: cfg.planAutoSwitch,
        defaultModel: cfg.defaultModel,
      });
      setCfg(res.config);
      toast.success(t("saved"));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSavingCfg(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-card px-4 py-3">
        <div>
          <div className="text-sm font-semibold">{t("serviceTitle")}</div>
          <div className="text-xs text-muted-foreground">{t("serviceSubtitle")}</div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {!running ? (
            <Button
              size="sm"
              disabled={!!busy["service"] || serviceAccounts.length === 0}
              onClick={() => void proxyStart()}
            >
              <Play className="h-4 w-4" />
              {t("serviceStart")}
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              disabled={!!busy["service"]}
              onClick={() => void proxyStop()}
            >
              <Square className="h-4 w-4" />
              {t("serviceStop")}
            </Button>
          )}
          <Button
            size="sm"
            variant="outline"
            disabled={!!busy["service"]}
            onClick={() => void proxyRestart()}
          >
            <RotateCw className="h-4 w-4" />
            {t("serviceRestart")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={!!busy["proxyTest"] || !running}
            onClick={() => void proxyTest()}
          >
            <FlaskConical className="h-4 w-4" />
            {t("serviceTest")}
          </Button>
          {state.proxy.processAlive && (
            <Button
              size="sm"
              variant="destructive"
              disabled={!!busy["service"]}
              onClick={() => void proxyKill()}
            >
              <Skull className="h-4 w-4" />
              {t("serviceKill")}
            </Button>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">{t("serviceAccount")}</CardTitle>
              <Badge variant={running ? "success" : "secondary"}>
                <StatusDot
                  tone={running ? "running" : state.proxy.processAlive ? "warn" : "stopped"}
                />
                <span className="ml-1.5">
                  {running
                    ? t("statusRunning")
                    : state.proxy.processAlive
                      ? t("statusServerStopped")
                      : t("statusProcessDown")}
                </span>
              </Badge>
            </div>
            <CardDescription className="text-xs">
              Yalnızca ZCode OAuth / Start Plan köprülemesi gerektiğinde kullanılır
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="space-y-1.5">
              <Label>{t("serviceAccount")}</Label>
              <Select
                value={activeId}
                onChange={(e) => void activateServiceAccount(e.target.value)}
                disabled={!!busy["service"] || serviceAccounts.length === 0}
              >
                {serviceAccounts.length === 0 ? (
                  <option value="">Hesaplar sekmesinden ZCode/Z.AI hesabı ekleyin</option>
                ) : (
                  serviceAccounts.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name} ({a.kind})
                    </option>
                  ))
                )}
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Endpoint</Label>
              <CopyField value={state.proxy.endpoint} />
            </div>
            {cfg?.proxyApiKey && (
              <div className="space-y-1.5">
                <Label>Yerel Servis Anahtarı (`x-api-key`)</Label>
                <CopyField value={cfg.proxyApiKey} mask />
              </div>
            )}
            <div className="pt-2 space-y-2 border-t">
              <div className="flex items-center justify-between">
                <Label className="font-normal text-muted-foreground">{t("proxyAutostart")}</Label>
                <Switch
                  checked={draft.proxyAutostart}
                  onCheckedChange={(v) => onDraftChange("proxyAutostart", v)}
                />
              </div>
              <div className="flex items-center justify-between">
                <Label className="font-normal text-muted-foreground">{t("keepProxyOnExit")}</Label>
                <Switch
                  checked={draft.keepProxyOnExit}
                  onCheckedChange={(v) => onDraftChange("keepProxyOnExit", v)}
                />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-base">Plan, Sağlayıcı & Proxy Yapılandırması</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div className="space-y-1">
                <Label>{t("defaultProvider")}</Label>
                <Select
                  value={draft.defaultProvider}
                  onChange={(e) =>
                    onDraftChange("defaultProvider", e.target.value as ProviderId)
                  }
                >
                  <option value="zai">Z.AI</option>
                  <option value="bigmodel">BigModel</option>
                </Select>
              </div>
              <div className="space-y-1">
                <Label>{t("defaultPlan")}</Label>
                <Select
                  value={draft.defaultPlan}
                  onChange={(e) =>
                    onDraftChange("defaultPlan", e.target.value as PlanId)
                  }
                >
                  <option value="coding-plan">{t("quotaCodingPlan")}</option>
                  <option value="start-plan">{t("quotaStartPlan")}</option>
                </Select>
              </div>
            </div>

            {cfg && (
              <>
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <div className="space-y-1">
                    <Label>Aktif Plan</Label>
                    <Select
                      value={cfg.plan}
                      onChange={(e) =>
                        setCfg({ ...cfg, plan: e.target.value as PlanId })
                      }
                    >
                      <option value="coding-plan">Coding Plan</option>
                      <option value="start-plan">Start Plan</option>
                    </Select>
                  </div>
                  <div className="space-y-1">
                    <Label>Port</Label>
                    <Input
                      type="number"
                      value={cfg.port}
                      onChange={(e) =>
                        setCfg({ ...cfg, port: Number(e.target.value) || 8317 })
                      }
                    />
                  </div>
                </div>
                <div className="space-y-1">
                  <Label>{t("defaultModel")}</Label>
                  <Input
                    className="font-mono text-xs"
                    value={cfg.defaultModel}
                    onChange={(e) => setCfg({ ...cfg, defaultModel: e.target.value })}
                  />
                </div>
                <div className="flex items-center justify-between py-1">
                  <Label className="font-normal text-muted-foreground">
                    {t("autoSwitch")}
                  </Label>
                  <Switch
                    checked={cfg.planAutoSwitch}
                    onCheckedChange={(v) => setCfg({ ...cfg, planAutoSwitch: v })}
                  />
                </div>
                <div className="flex justify-end">
                  <Button
                    size="sm"
                    disabled={savingCfg}
                    onClick={() => void savePlanConfig()}
                  >
                    Proxy Ayarını Kaydet
                  </Button>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      </div>

      <QuotaPanel
        quota={quota}
        loading={quotaLoading}
        error={quotaError}
        proxyOff={!running}
        onRefresh={() => void fetchQuota()}
      />

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("recentRequests")}</CardTitle>
          </CardHeader>
          <CardContent>
            {logs.length === 0 ? (
              <div className="text-sm text-muted-foreground">{t("noRequestsYet")}</div>
            ) : (
              <pre className="max-h-60 overflow-y-auto rounded border bg-muted/40 p-2 font-mono text-[11px] leading-relaxed">
                {logs.slice(-60).join("\n")}
              </pre>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("recentErrors")}</CardTitle>
          </CardHeader>
          <CardContent>
            {errorLogs.length === 0 ? (
              <div className="text-sm text-muted-foreground">{t("noErrors")}</div>
            ) : (
              <pre className="max-h-60 overflow-y-auto rounded border bg-muted/40 p-2 font-mono text-[11px] leading-relaxed">
                {errorLogs
                  .slice(-25)
                  .map((e) => JSON.stringify(e))
                  .join("\n")}
              </pre>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
