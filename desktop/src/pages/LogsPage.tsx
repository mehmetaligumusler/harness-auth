import * as React from "react";
import { AlertOctagon, Radio } from "lucide-react";
import { Badge, Button, Card, CardContent, Tabs, TabsList, TabsTrigger } from "@/components/ui";
import { EmptyState, PageHeader } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import { formatDateTime } from "@/lib/utils";

const KIND_LABELS: Record<string, string> = {
  request_error: "İstek hatası",
  error_response: "Hata yanıtı",
  plan_fallback: "Plan geçişi",
  plan_quota_exhausted: "Kota tükendi",
  upstream_gateway_retry: "Gateway yeniden deneme",
  upstream_connect_retry: "Bağlantı yeniden deneme",
  upstream_postwrite_failure: "Upstream yazma hatası",
  client_gone_before_connect: "İstemci iptal etti",
  stream_collect_failed: "Akış toplama hatası",
};

export function LogsPage() {
  const t = useT();
  const logs = useApp((s) => s.logs);
  const logsFollow = useApp((s) => s.logsFollow);
  const setLogsFollow = useApp((s) => s.setLogsFollow);
  const pollLogs = useApp((s) => s.pollLogs);
  const resetLogs = useApp((s) => s.resetLogs);
  const errorLogs = useApp((s) => s.errorLogs);
  const fetchErrorLogs = useApp((s) => s.fetchErrorLogs);
  const proxyAlive = useApp((s) => !!s.state?.proxy.processAlive);

  const [tab, setTab] = React.useState("live");
  const [levelFilter, setLevelFilter] = React.useState<"all" | "info" | "warn" | "error">("all");

  React.useEffect(() => {
    if (tab === "live" && proxyAlive) {
      resetLogs();
      void pollLogs();
      const id = setInterval(() => {
        if (useApp.getState().logsFollow) void pollLogs();
      }, 2000);
      return () => clearInterval(id);
    }
  }, [tab, proxyAlive, pollLogs, resetLogs]);

  React.useEffect(() => {
    if (tab === "errors") void fetchErrorLogs(300);
  }, [tab, fetchErrorLogs]);

  const classify = (line: string): "info" | "warn" | "error" => {
    const l = line.toLowerCase();
    if (l.includes("error") || /\b4\d\d\b|\b5\d\d\b/.test(l)) {
      if (l.includes("warn")) return "warn";
      return "error";
    }
    if (l.includes("warn") || l.includes("fallback")) return "warn";
    return "info";
  };

  const filteredLogs = logs.filter((line) =>
    levelFilter === "all" ? true : classify(line) === levelFilter,
  );

  const filteredErrors = errorLogs.filter((row) => {
    if (levelFilter === "all") return true;
    const status = Number(row.status ?? 0);
    if (levelFilter === "error") return status >= 500 || String(row.kind ?? "").includes("stream");
    if (levelFilter === "warn") return status >= 400 && status < 500;
    return true;
  });

  return (
    <>
      <PageHeader title={t("logsTitle")} subtitle={t("logsSubtitle")} />

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Tabs value={tab} onValueChange={setTab}>
          <TabsList>
            <TabsTrigger value="live">
              <Radio className="mr-1.5 h-3.5 w-3.5" /> {t("logsTabLive")}
            </TabsTrigger>
            <TabsTrigger value="errors">
              <AlertOctagon className="mr-1.5 h-3.5 w-3.5" /> {t("logsTabErrors")}
            </TabsTrigger>
          </TabsList>
        </Tabs>
        <Tabs value={levelFilter} onValueChange={(v) => setLevelFilter(v as typeof levelFilter)}>
          <TabsList>
            <TabsTrigger value="all">{t("logsFilterAll")}</TabsTrigger>
            <TabsTrigger value="info">{t("logsFilterInfo")}</TabsTrigger>
            <TabsTrigger value="warn">{t("logsFilterWarn")}</TabsTrigger>
            <TabsTrigger value="error">{t("logsFilterError")}</TabsTrigger>
          </TabsList>
        </Tabs>
        {tab === "live" && (
          <div className="ml-auto flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setLogsFollow(!logsFollow)}
            >
              {logsFollow ? t("logsPaused") : t("logsFollow")}
            </Button>
            <Button variant="ghost" size="sm" onClick={resetLogs}>
              {t("logsClear")}
            </Button>
          </div>
        )}
      </div>

      {tab === "live" ? (
        <Card>
          <CardContent className="p-0">
            <p className="border-b px-4 py-2 text-xs text-muted-foreground">{t("logsLiveNote")}</p>
            {filteredLogs.length === 0 ? (
              <EmptyState title={t("logsEmpty")} />
            ) : (
              <div className="max-h-[calc(100vh-320px)] overflow-y-auto p-4 font-mono text-[11.5px] leading-relaxed">
                {filteredLogs
                  .slice()
                  .reverse()
                  .map((line, i) => (
                    <div
                      key={i}
                      className={
                        classify(line) === "error"
                          ? "text-destructive"
                          : classify(line) === "warn"
                            ? "text-warning"
                            : "text-muted-foreground"
                      }
                    >
                      {line}
                    </div>
                  ))}
              </div>
            )}
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardContent className="p-0">
            {filteredErrors.length === 0 ? (
              <EmptyState title={t("errorsEmpty")} />
            ) : (
              <div className="max-h-[calc(100vh-320px)] divide-y overflow-y-auto">
                {filteredErrors
                  .slice()
                  .reverse()
                  .map((row, i) => (
                    <div key={i} className="flex flex-wrap items-center gap-3 px-4 py-2.5 text-xs">
                      <span className="w-40 shrink-0 text-muted-foreground">
                        {formatDateTime(String(row.time ?? ""))}
                      </span>
                      <Badge
                        variant={
                          Number(row.status) >= 500 || String(row.kind).includes("stream")
                            ? "destructive"
                            : "warning"
                        }
                      >
                        {KIND_LABELS[String(row.kind ?? "")] ?? String(row.kind ?? "?")}
                      </Badge>
                      {row.status != null && (
                        <span className="font-mono font-semibold">{String(row.status)}</span>
                      )}
                      {row.model != null && (
                        <span className="font-mono text-muted-foreground">
                          {String(row.model)}
                        </span>
                      )}
                      {row.plan != null && (
                        <span className="text-muted-foreground">{String(row.plan)}</span>
                      )}
                      {row.error != null && (
                        <span className="min-w-0 flex-1 truncate text-muted-foreground">
                          {String(row.error)}
                        </span>
                      )}
                    </div>
                  ))}
              </div>
            )}
          </CardContent>
        </Card>
      )}
    </>
  );
}
