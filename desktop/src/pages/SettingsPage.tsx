import * as React from "react";
import { FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Label,
  Select,
  Switch,
} from "@/components/ui";
import { KeyValue, PageHeader, SectionTitle } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import type { Settings as SettingsType } from "@/lib/types";

export function SettingsPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const saveSettings = useApp((s) => s.saveSettings);
  const busy = useApp((s) => !!s.busy["settings"]);

  const [draft, setDraft] = React.useState<SettingsType | null>(null);

  React.useEffect(() => {
    if (state?.settings && !draft) setDraft({ ...state.settings });
  }, [state?.settings, draft]);

  if (!state || !draft) {
    return <div className="py-24 text-center text-muted-foreground">{t("loading")}</div>;
  }

  const set = <K extends keyof SettingsType>(key: K, value: SettingsType[K]) =>
    setDraft((d) => (d ? { ...d, [key]: value } : d));

  const save = async () => {
    await saveSettings(draft);
  };

  const pickDefaultFolder = async () => {
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") set("claudeDefaultFolder", picked);
    } catch {
      /* cancelled */
    }
  };

  const Toggle = ({
    labelKey,
    checked,
    onChange,
  }: {
    labelKey: Parameters<ReturnType<typeof useT>>[0];
    checked: boolean;
    onChange: (v: boolean) => void;
  }) => (
    <div className="flex items-center justify-between py-2">
      <Label className="text-sm font-normal text-muted-foreground">{t(labelKey)}</Label>
      <Switch checked={checked} onCheckedChange={onChange} />
    </div>
  );

  return (
    <>
      <PageHeader
        title={t("settingsTitle")}
        subtitle={t("settingsSubtitle")}
        actions={
          <Button size="sm" disabled={busy} onClick={() => void save()}>
            {t("save")}
          </Button>
        }
      />

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* General */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("settingsGeneral")}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="set-theme">{t("theme")}</Label>
              <Select
                id="set-theme"
                value={draft.theme}
                onChange={(e) => set("theme", e.target.value as SettingsType["theme"])}
              >
                <option value="dark">{t("themeDark")}</option>
                <option value="light">{t("themeLight")}</option>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="set-lang">{t("language")}</Label>
              <Select
                id="set-lang"
                value={draft.language}
                onChange={(e) => set("language", e.target.value as SettingsType["language"])}
              >
                <option value="tr">Türkçe</option>
                <option value="en">English</option>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="set-log">{t("logLevel")}</Label>
              <Select
                id="set-log"
                value={draft.logLevel}
                onChange={(e) => set("logLevel", e.target.value)}
              >
                <option value="info">info</option>
                <option value="debug">debug</option>
              </Select>
            </div>
            <Toggle
              labelKey="updateCheck"
              checked={draft.updateCheck}
              onChange={(v) => set("updateCheck", v)}
            />
          </CardContent>
        </Card>

        {/* Defaults */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("settingsDefaults")}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="set-provider">{t("defaultProvider")}</Label>
              <Select
                id="set-provider"
                value={draft.defaultProvider}
                onChange={(e) =>
                  set("defaultProvider", e.target.value as SettingsType["defaultProvider"])
                }
              >
                <option value="zai">Z.AI</option>
                <option value="bigmodel">BigModel</option>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="set-plan">{t("defaultPlanLabel")}</Label>
              <Select
                id="set-plan"
                value={draft.defaultPlan}
                onChange={(e) => set("defaultPlan", e.target.value as SettingsType["defaultPlan"])}
              >
                <option value="coding-plan">{t("planCoding")}</option>
                <option value="start-plan">{t("planStart")}</option>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="set-model">{t("defaultModelLabel")}</Label>
              <Input
                id="set-model"
                className="font-mono"
                value={draft.defaultModel}
                onChange={(e) => set("defaultModel", e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label>{t("claudeDefaultFolderLabel")}</Label>
              <div className="flex gap-2">
                <Input
                  value={draft.claudeDefaultFolder ?? ""}
                  onChange={(e) => set("claudeDefaultFolder", e.target.value)}
                  className="font-mono"
                  placeholder="C:\\Projects"
                />
                <Button variant="outline" onClick={() => void pickDefaultFolder()}>
                  <FolderOpen />
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Proxy behavior */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("settingsProxy")}</CardTitle>
          </CardHeader>
          <CardContent>
            <Toggle
              labelKey="proxyAutoStart"
              checked={draft.proxyAutostart}
              onChange={(v) => set("proxyAutostart", v)}
            />
            <Toggle
              labelKey="keepProxyOnExit"
              checked={draft.keepProxyOnExit}
              onChange={(v) => set("keepProxyOnExit", v)}
            />
          </CardContent>
        </Card>

        {/* Windows */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t("settingsWindows")}</CardTitle>
          </CardHeader>
          <CardContent>
            <Toggle
              labelKey="runAtStartup"
              checked={draft.autostartWindows}
              onChange={(v) => set("autostartWindows", v)}
            />
            <Toggle
              labelKey="minimizeToTray"
              checked={draft.closeToTray}
              onChange={(v) => set("closeToTray", v)}
            />
            <Toggle
              labelKey="runInBackground"
              checked={draft.keepProxyOnExit}
              onChange={(v) => set("keepProxyOnExit", v)}
            />
            <Toggle
              labelKey="startMinimizedLabel"
              checked={draft.startMinimized}
              onChange={(v) => set("startMinimized", v)}
            />
          </CardContent>
        </Card>
      </div>

      {/* About */}
      <Card className="mt-4">
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t("settingsAbout")}</CardTitle>
        </CardHeader>
        <CardContent>
          <KeyValue label={t("versionApp")} value={state.versions.app} />
          <KeyValue label={t("versionProxy")} value={state.versions.proxy ?? t("unknown")} />
          <div className="mt-2">
            <SectionTitle>{t("openDataFolder")}</SectionTitle>
            <code className="block break-all rounded-md border bg-muted/50 px-2 py-1.5 font-mono text-xs">
              {state.paths.dataDir}
            </code>
          </div>
          <div className="mt-3 flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
                  await revealItemInDir(state.paths.config);
                } catch (e) {
                  toast.error(t("errGeneric"), { description: String(e) });
                }
              }}
            >
              {t("openConfigFile")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
                  await revealItemInDir(state.paths.dataDir);
                } catch (e) {
                  toast.error(t("errGeneric"), { description: String(e) });
                }
              }}
            >
              {t("openDataFolder")}
            </Button>
          </div>
        </CardContent>
      </Card>
    </>
  );
}
