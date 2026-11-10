// Quota rendering shared by the Dashboard and the Plans page.
// Only real API data is shown — anything the API does not provide renders as
// "Kullanılamıyor" / "Not available", never an invented number.

import { AlertTriangle } from "lucide-react";
import { Badge, Card, CardContent, CardHeader, CardTitle, Progress, Skeleton } from "@/components/ui";
import { EmptyState, KeyValue, SectionTitle } from "@/components/common";
import { useT } from "@/stores/app";
import type { QuotaLimit, QuotaSnapshot } from "@/lib/types";
import { formatDateTime } from "@/lib/utils";

const LIMIT_LABELS: Record<string, string> = {
  TOKENS_LIMIT: "quotaWindow5h",
  TIME_LIMIT: "quotaMonthly",
  WEEK_LIMIT: "quotaWeekly",
};

function LimitRow({ limit }: { limit: QuotaLimit }) {
  const t = useT();
  const labelKey = LIMIT_LABELS[limit.type] ?? null;
  const label = labelKey ? t(labelKey as never) : limit.type;
  const total = limit.total ?? null;
  const used = limit.used ?? null;
  const remaining = limit.remaining ?? null;
  const pct =
    limit.percentage ??
    (total && used != null && total > 0 ? Math.round((used / total) * 100) : null);

  return (
    <div className="py-2">
      <div className="mb-1 flex items-baseline justify-between gap-3">
        <span className="text-sm font-medium">{label}</span>
        <span className="text-xs text-muted-foreground">
          {used != null && total != null
            ? `${used.toLocaleString()} / ${total.toLocaleString()} ${limit.unit ?? ""}`
            : remaining != null
              ? `${t("quotaRemaining")}: ${remaining.toLocaleString()}`
              : t("quotaNotProvided")}
        </span>
      </div>
      {pct != null && <Progress value={pct} className="h-1.5" />}
      {limit.nextResetTime && (
        <div className="mt-1 text-[11px] text-muted-foreground">
          {t("quotaRefreshAt")}: {formatDateTime(limit.nextResetTime)}
        </div>
      )}
    </div>
  );
}

export function QuotaPanel({
  quota,
  loading,
  error,
  proxyOff,
  onRefresh,
}: {
  quota: QuotaSnapshot | null;
  loading: boolean;
  error: string | null;
  proxyOff: boolean;
  onRefresh?: () => void;
}) {
  const t = useT();

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="text-base">{t("quotaTitle")}</CardTitle>
        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            disabled={loading || proxyOff}
            className="text-xs text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
          >
            {loading ? t("loading") : t("quotaRefresh")}
          </button>
        )}
      </CardHeader>
      <CardContent>
        {proxyOff ? (
          <p className="py-6 text-center text-sm text-muted-foreground">{t("quotaProxyOff")}</p>
        ) : loading && !quota ? (
          <div className="space-y-3 py-2">
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="h-4 w-3/5" />
          </div>
        ) : error && !quota ? (
          <EmptyState
            icon={<AlertTriangle className="h-8 w-8" />}
            title={t("error")}
            description={error}
          />
        ) : quota ? (
          <div className="space-y-4">
            {quota.jwt?.ageHours != null && (
              <div className="text-xs text-muted-foreground">
                {t("quotaJwtAge")}: {Math.round(quota.jwt.ageHours)} sa
              </div>
            )}

            <div>
              <SectionTitle>{t("quotaCodingPlan")}</SectionTitle>
              {quota.codingPlan?.limits?.length ? (
                quota.codingPlan.limits.map((l) => <LimitRow key={l.type} limit={l} />)
              ) : (
                <p className="py-1 text-sm text-muted-foreground">{t("quotaNotProvided")}</p>
              )}
              {quota.codingPlan?.level && (
                <div className="mt-1">
                  <Badge variant="secondary">{quota.codingPlan.level}</Badge>
                </div>
              )}
            </div>

            <div>
              <SectionTitle>{t("quotaStartPlan")}</SectionTitle>
              {quota.balances?.length ? (
                quota.balances.map((b, i) => (
                  <div key={i} className="py-1">
                    <div className="mb-1 flex items-baseline justify-between gap-3">
                      <span className="text-sm font-medium">{b.showName}</span>
                      <span className="text-xs text-muted-foreground">
                        {t("quotaRemaining")}: {b.remainingUnits.toLocaleString()}{" "}
                        {b.unitType ?? "kredi"}
                      </span>
                    </div>
                    {b.totalUnits > 0 && (
                      <Progress
                        value={Math.round((b.usedUnits / b.totalUnits) * 100)}
                        className="h-1.5"
                      />
                    )}
                    {b.expiresAt && (
                      <div className="mt-1 text-[11px] text-muted-foreground">
                        {t("quotaExpires")}: {formatDateTime(b.expiresAt)}
                      </div>
                    )}
                  </div>
                ))
              ) : (
                <p className="py-1 text-sm text-muted-foreground">{t("quotaNotProvided")}</p>
              )}
            </div>

            {quota.claimablePlans && quota.claimablePlans.length > 0 && (
              <KeyValue
                label={t("quotaClaimable")}
                value={quota.claimablePlans.join(", ")}
              />
            )}

            {quota.errors && quota.errors.length > 0 && (
              <div className="rounded-md border border-warning/40 bg-warning/10 p-2 text-xs text-warning-foreground">
                {quota.errors.map((e, i) => (
                  <div key={i}>{e}</div>
                ))}
              </div>
            )}
          </div>
        ) : (
          <EmptyState icon={<AlertTriangle className="h-8 w-8" />} title={t("notAvailable")} />
        )}
      </CardContent>
    </Card>
  );
}
