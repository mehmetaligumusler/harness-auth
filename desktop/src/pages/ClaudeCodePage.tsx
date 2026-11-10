import * as React from "react";
import { FolderOpen, Monitor, Terminal, Zap } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Label,
  Select,
} from "@/components/ui";
import { KeyValue, PageHeader } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import { api } from "@/lib/api";
import { MODEL_CATALOG } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export function ClaudeCodePage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const [mode, setMode] = React.useState<"proxy" | "normal">("proxy");
  const [model, setModel] = React.useState<string>("");
  const [folder, setFolder] = React.useState<string>("");
  const [launching, setLaunching] = React.useState(false);

  const claude = state?.claude;
  const proxy = state?.proxy;
  const running = !!proxy?.processAlive && !!proxy?.serverRunning;
  const account = state?.accounts.find((a) => a.id === proxy?.activeAccountId);

  React.useEffect(() => {
    if (!model && state?.settings.defaultModel) setModel(state.settings.defaultModel);
  }, [state?.settings.defaultModel, model]);

  React.useEffect(() => {
    if (!folder && state?.settings.claudeDefaultFolder)
      setFolder(state.settings.claudeDefaultFolder);
    else if (!folder && state?.settings.recentFolders[0])
      setFolder(state.settings.recentFolders[0]);
  }, [state?.settings.claudeDefaultFolder, state?.settings.recentFolders, folder]);

  const pickFolder = async () => {
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") setFolder(picked);
    } catch {
      /* dialog cancelled */
    }
  };

  const launch = async () => {
    if (mode === "proxy" && !running) {
      toast.error(t("claudeNeedsProxy"));
      return;
    }
    setLaunching(true);
    try {
      await api.claudeLaunch({ mode, model: model || undefined, folder: folder || undefined });
    } catch (e) {
      toast.error(t("errGeneric"), { description: String(e) });
    } finally {
      setLaunching(false);
    }
  };

  const modelOptions = Array.from(
    new Set([
      ...MODEL_CATALOG.map((c) => c.id),
      ...(state?.settings.recentModels ?? []),
      ...(account?.defaultModel ? [account.defaultModel] : []),
    ]),
  );

  return (
    <>
      <PageHeader title={t("claudeTitle")} subtitle={t("claudeSubtitle")} />

      {/* Detection */}
      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <Terminal className="h-4 w-4 text-muted-foreground" />
            Claude Code CLI
            {claude?.found ? (
              <Badge variant="success">{t("claudeDetected")}</Badge>
            ) : (
              <Badge variant="warning">{t("claudeNotDetected")}</Badge>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {claude?.found ? (
            <>
              <KeyValue label={t("claudeVersion")} value={claude.version ?? t("unknown")} />
              <KeyValue label={t("claudePath")} value={claude.path ?? "—"} mono />
            </>
          ) : (
            <p className="rounded-md border border-warning/40 bg-warning/10 p-3 font-mono text-xs text-warning-foreground">
              {t("claudeInstallHint")}
            </p>
          )}
        </CardContent>
      </Card>

      {/* Mode selection */}
      <div className="mb-4 grid grid-cols-1 gap-4 md:grid-cols-2">
        {(
          [
            {
              id: "proxy" as const,
              icon: <Zap className="h-4 w-4 text-primary" />,
              title: t("launchModeProxy"),
              desc: t("launchModeProxyDesc"),
            },
            {
              id: "normal" as const,
              icon: <Monitor className="h-4 w-4 text-muted-foreground" />,
              title: t("launchModeNormal"),
              desc: t("launchModeNormalDesc"),
            },
          ]
        ).map((m) => (
          <button
            key={m.id}
            type="button"
            onClick={() => setMode(m.id)}
            className={cn(
              "rounded-lg border p-4 text-left transition-colors",
              mode === m.id
                ? "border-primary bg-primary/5 ring-1 ring-primary"
                : "bg-card hover:bg-accent/50",
            )}
          >
            <div className="mb-1 flex items-center gap-2 font-medium">
              {m.icon} {m.title}
            </div>
            <p className="text-xs text-muted-foreground">{m.desc}</p>
          </button>
        ))}
      </div>

      {/* Launch configuration */}
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t("launch")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {mode === "proxy" && (
            <>
              <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                <div className="space-y-1.5">
                  <Label>{t("selectAccount")}</Label>
                  <div className="rounded-md border px-3 py-2 text-sm">
                    {account ? (
                      <span>
                        {account.name}{" "}
                        <span className="text-muted-foreground">
                          ({account.kind === "oauth" ? "OAuth" : "API Key"})
                        </span>
                      </span>
                    ) : (
                      <span className="text-muted-foreground">{t("errNoActiveAccount")}</span>
                    )}
                  </div>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="cc-model">{t("selectModel")}</Label>
                  <Select
                    id="cc-model"
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    className="font-mono"
                  >
                    {modelOptions.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </Select>
                </div>
              </div>
              {!running && (
                <p className="text-xs text-warning">{t("claudeNeedsProxy")}</p>
              )}
            </>
          )}

          <div className="space-y-1.5">
            <Label htmlFor="cc-folder">{t("projectFolder")}</Label>
            <div className="flex gap-2">
              <Input
                id="cc-folder"
                value={folder}
                onChange={(e) => setFolder(e.target.value)}
                placeholder="C:\\Projects\\my-app"
                className="font-mono"
              />
              <Button variant="outline" onClick={() => void pickFolder()}>
                <FolderOpen /> {t("pickFolder")}
              </Button>
            </div>
            {(state?.settings.recentFolders?.length ?? 0) > 0 && (
              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                <span className="text-xs text-muted-foreground">{t("recentFolders")}:</span>
                {state?.settings.recentFolders.slice(0, 5).map((f) => (
                  <button
                    key={f}
                    type="button"
                    onClick={() => setFolder(f)}
                    className="rounded border bg-muted/50 px-1.5 py-0.5 font-mono text-[11px] hover:bg-accent"
                    title={f}
                  >
                    {f.length > 40 ? `…${f.slice(-38)}` : f}
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="flex items-center justify-between gap-4">
            <p className="text-xs text-muted-foreground">{t("claudeEnvNote")}</p>
            <Button
              size="lg"
              disabled={launching || !claude?.found || (mode === "proxy" && !running)}
              onClick={() => void launch()}
            >
              <Terminal /> {t("launch")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </>
  );
}
