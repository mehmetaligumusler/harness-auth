import * as React from "react";
import { Play, PowerOff, RotateCw, ShieldCheck, Square, Zap } from "lucide-react";
import { toast } from "sonner";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Input, Label, Select } from "@/components/ui";
import { KeyValue, PageHeader, StatusDot } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import { api } from "@/lib/api";

export function ProxyPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const busy = useApp((s) => !!s.busy["proxy"]);
  const testBusy = useApp((s) => !!s.busy["proxyTest"]);
  const proxyStart = useApp((s) => s.proxyStart);
  const proxyStop = useApp((s) => s.proxyStop);
  const proxyRestart = useApp((s) => s.proxyRestart);
  const proxyKill = useApp((s) => s.proxyKill);
  const proxyTest = useApp((s) => s.proxyTest);

  const [port, setPort] = React.useState("");
  const [host, setHost] = React.useState("127.0.0.1");
  const [apiKey, setApiKey] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [testResult, setTestResult] = React.useState<string | null>(null);

  const proxy = state?.proxy;
  const running = !!proxy?.processAlive && !!proxy?.serverRunning;

  React.useEffect(() => {
    api
      .getConfig()
      .then((c) => {
        setPort(String(c.port));
        setHost(c.host);
        setApiKey(c.proxyApiKey ?? "");
      })
      .catch(() => undefined);
  }, [state?.proxy.loggedIn]);

  const saveServer = async () => {
    setSaving(true);
    try {
      const res = await api.saveConfig({
        port: Number(port) || undefined,
        host,
        ...(apiKey.trim().length >= 8 || apiKey.trim() === "" ? { proxyApiKey: apiKey.trim() } : {}),
      });
      setPort(String(res.config.port));
      setHost(res.config.host);
      setApiKey(res.config.proxyApiKey ?? "");
      toast.success(res.restarted ? "Proxy yeniden başlatıldı" : t("saved"));
    } catch (e) {
      toast.error(t("errGeneric"), { description: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const regenerateKey = () => {
    const chars = "abcdefghijkmnopqrstuvwxyz023456789";
    let out = "";
    const arr = new Uint32Array(32);
    crypto.getRandomValues(arr);
    for (let i = 0; i < 32; i++) out += chars[arr[i] % chars.length];
    setApiKey(out);
  };

  return (
    <>
      <PageHeader
        title={t("proxyTitle")}
        subtitle={t("proxySubtitle")}
        actions={
          <Button variant="outline" size="sm" disabled={testBusy} onClick={() => void proxyTest()}>
            <Zap /> {t("proxyTestConnection")}
          </Button>
        }
      />

      {testResult && (
        <div className="mb-4 rounded-md border bg-muted/40 px-4 py-2.5 text-sm">{testResult}</div>
      )}

      {/* Status + controls */}
      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <StatusDot tone={running ? "running" : proxy?.processAlive ? "warn" : "stopped"} />
            {t("proxyStatusLine")}
            {proxy?.adopted && <Badge variant="secondary">{t("statusAdopted")}</Badge>}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-x-8 md:grid-cols-2">
            <div>
              <KeyValue
                label={t("proxyProcessSection")}
                value={proxy?.processAlive ? t("statusRunning") : t("statusProcessDown")}
              />
              <KeyValue
                label={t("proxyServerSection")}
                value={running ? t("statusServerRunning") : t("statusServerStopped")}
              />
            </div>
            <div>
              <KeyValue label={t("proxyEndpoint")} value={proxy?.endpoint ?? "—"} mono />
              <KeyValue label="Panel portu" value={proxy?.panelPort ?? "—"} />
            </div>
          </div>

          {proxy?.adopted && (
            <p className="mt-3 rounded-md border border-warning/40 bg-warning/10 p-2.5 text-xs text-warning-foreground">
              {t("proxyAdoptedNote")}
            </p>
          )}

          <div className="mt-4 flex flex-wrap gap-2">
            {running ? (
              <Button size="sm" variant="destructive" disabled={busy} onClick={proxyStop}>
                <Square /> {t("proxyStopServer")}
              </Button>
            ) : (
              <Button size="sm" disabled={busy} onClick={proxyStart}>
                <Play /> {t("proxyStartServer")}
              </Button>
            )}
            <Button
              size="sm"
              variant="outline"
              disabled={busy || !proxy?.processAlive}
              onClick={proxyRestart}
            >
              <RotateCw /> {t("proxyRestartProcess")}
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="text-destructive hover:text-destructive"
              disabled={busy || !proxy?.processAlive}
              onClick={proxyKill}
            >
              <PowerOff /> {t("proxyKillProcess")}
            </Button>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">{t("proxyProcessNote")}</p>
        </CardContent>
      </Card>

      {/* Server config */}
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <ShieldCheck className="h-4 w-4 text-muted-foreground" />
            {t("proxyServerSection")}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
            <div className="space-y-1.5">
              <Label htmlFor="srv-port">{t("proxyPort")}</Label>
              <Input
                id="srv-port"
                value={port}
                onChange={(e) => setPort(e.target.value)}
                inputMode="numeric"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="srv-host">{t("proxyHost")}</Label>
              <Select id="srv-host" value={host} onChange={(e) => setHost(e.target.value)}>
                <option value="127.0.0.1">127.0.0.1 (yalnızca bu bilgisayar)</option>
                <option value="localhost">localhost</option>
                <option value="::1">::1</option>
                <option value="0.0.0.0">0.0.0.0 (tüm arayüzler — dikkat)</option>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="srv-key">{t("proxyApiKey")}</Label>
              <div className="flex gap-2">
                <Input
                  id="srv-key"
                  type="password"
                  autoComplete="off"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  className="font-mono"
                  placeholder="(kapalı)"
                />
                <Button variant="outline" size="sm" onClick={regenerateKey}>
                  {t("proxyRegenerateKey")}
                </Button>
              </div>
            </div>
          </div>
          <p className="text-xs text-muted-foreground">{t("proxyApiKeyHint")}</p>
          <div className="flex items-center justify-between">
            <p className="text-xs text-muted-foreground">{t("portChangeNote")}</p>
            <Button size="sm" disabled={saving} onClick={() => void saveServer()}>
              {t("save")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </>
  );
}
