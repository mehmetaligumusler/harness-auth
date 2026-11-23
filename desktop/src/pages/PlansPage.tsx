import * as React from "react";
import { ArrowRight, Layers, Repeat, Zap } from "lucide-react";
import { toast } from "sonner";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Select } from "@/components/ui";
import { PageHeader, SectionTitle } from "@/components/common";
import { QuotaPanel } from "@/components/QuotaPanel";
import { useApp, useT } from "@/stores/app";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { PlanId, ProxyConfig } from "@/lib/types";

export function PlansPage({ embedded }: { embedded?: boolean } = {}) {
  const t = useT();
  const state = useApp((s) => s.state);
  const quota = useApp((s) => s.quota);
  const quotaLoading = useApp((s) => s.quotaLoading);
  const quotaError = useApp((s) => s.quotaError);
  const fetchQuota = useApp((s) => s.fetchQuota);
  const [cfg, setCfg] = React.useState<ProxyConfig | null>(null);
  const [saving, setSaving] = React.useState(false);

  const load = React.useCallback(() => {
    api.getConfig().then(setCfg).catch(() => setCfg(null));
  }, []);

  React.useEffect(() => {
    load();
  }, [load, state?.proxy.loggedIn]);

  React.useEffect(() => {
    if (state?.proxy.processAlive) void fetchQuota();
  }, [state?.proxy.processAlive, fetchQuota]);

  const account = state?.accounts.find((a) => a.id === state?.proxy.activeAccountId);
  const isApiKey = account?.kind === "api-key";
  const hasJwt = !!account?.hasJwt;

  const mode: "coding" | "start" | "auto" = cfg?.planAutoSwitch
    ? "auto"
    : cfg?.plan === "start-plan"
      ? "start"
      : "coding";

  const apply = async (nextMode: "coding" | "start" | "auto", priority?: PlanId[]) => {
    if (!cfg) return;
    if (nextMode === "start" && (!hasJwt || isApiKey)) {
      toast.error(t("planNoJwt"));
      return;
    }
    setSaving(true);
    try {
      const res = await api.saveConfig({
        plan: nextMode === "start" ? "start-plan" : "coding-plan",
        planAutoSwitch: nextMode === "auto",
        ...(priority ? { planPriority: priority } : {}),
      });
      setCfg(res.config);
      toast.success(t("planLiveNote"));
    } catch (e) {
      toast.error(t("errGeneric"), { description: typeof e === "string" ? e : String(e) });
    } finally {
      setSaving(false);
    }
  };

  const running = !!state?.proxy.processAlive;

  const ModeCard = ({
    id,
    icon,
    title,
    desc,
    disabled,
    disabledNote,
  }: {
    id: "coding" | "start" | "auto";
    icon: React.ReactNode;
    title: string;
    desc: string;
    disabled?: boolean;
    disabledNote?: string;
  }) => (
    <button
      type="button"
      disabled={disabled || saving}
      onClick={() => void apply(id)}
      className={cn(
        "rounded-lg border p-4 text-left transition-colors",
        mode === id
          ? "border-primary bg-primary/5 ring-1 ring-primary"
          : "bg-card hover:bg-accent/50",
        disabled && "cursor-not-allowed opacity-50",
      )}
    >
      <div className="mb-1 flex items-center gap-2">
        {icon}
        <span className="font-medium">{title}</span>
        {mode === id && <Badge variant="default">{t("active")}</Badge>}
      </div>
      <p className="text-xs text-muted-foreground">{desc}</p>
      {disabled && disabledNote && (
        <p className="mt-1.5 text-xs text-warning">{disabledNote}</p>
      )}
    </button>
  );

  return (
    <>
      {!embedded && <PageHeader title={t("plansTitle")} subtitle={t("plansSubtitle")} />}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <ModeCard
          id="coding"
          icon={<Layers className="h-4 w-4 text-primary" />}
          title={t("planCoding")}
          desc={t("planCodingDesc")}
        />
        <ModeCard
          id="start"
          icon={<Zap className="h-4 w-4 text-warning" />}
          title={t("planStart")}
          desc={t("planStartDesc")}
          disabled={isApiKey || !hasJwt}
          disabledNote={isApiKey ? t("planApiKeyLimit") : !hasJwt ? t("planNoJwt") : undefined}
        />
        <ModeCard
          id="auto"
          icon={<Repeat className="h-4 w-4 text-success" />}
          title={t("planAuto")}
          desc={t("planAutoDesc")}
          disabled={isApiKey}
          disabledNote={isApiKey ? t("planApiKeyLimit") : undefined}
        />
      </div>

      {mode === "auto" && cfg && (
        <Card className="mt-4">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("planPriority")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="mb-3 text-xs text-muted-foreground">{t("planPriorityNote")}</p>
            <div className="flex max-w-md items-center gap-3">
              <Select
                value={cfg.planPriority[0]}
                disabled={saving}
                onChange={(e) => {
                  const first = e.target.value as PlanId;
                  const second = first === "coding-plan" ? "start-plan" : "coding-plan";
                  void apply("auto", [first, second]);
                }}
                className="w-48"
              >
                <option value="coding-plan">{t("planCoding")}</option>
                <option value="start-plan">{t("planStart")}</option>
              </Select>
              <ArrowRight className="h-4 w-4 text-muted-foreground" />
              <span className="text-sm text-muted-foreground">
                {cfg.planPriority[1] === "coding-plan" ? t("planCoding") : t("planStart")}
              </span>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <QuotaPanel
          quota={quota}
          loading={quotaLoading}
          error={quotaError}
          proxyOff={!running}
          onRefresh={() => void fetchQuota()}
        />
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("account")}</CardTitle>
          </CardHeader>
          <CardContent>
            {account ? (
              <>
                <SectionTitle>{account.name}</SectionTitle>
                <p className="text-sm text-muted-foreground">
                  {account.kind === "oauth" ? "OAuth" : "API Key"} ·{" "}
                  {account.provider === "zai" ? "Z.AI" : "BigModel"} ·{" "}
                  {account.hasJwt ? "Start Plan JWT ✓" : "Start Plan JWT ✗"}
                </p>
              </>
            ) : (
              <p className="text-sm text-muted-foreground">{t("errNoActiveAccount")}</p>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
