import * as React from "react";
import { FolderOpen, RotateCcw } from "lucide-react";
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
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { ConfirmDialog, KeyValue, PageHeader, SectionTitle } from "@/components/common";
import { ZCodeServicePanel } from "@/components/ZCodeServicePanel";
import { api } from "@/lib/api";
import { useApp, useT } from "@/stores/app";
import type { Settings as SettingsType } from "@/lib/types";

export function SettingsPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const saveSettings = useApp((s) => s.saveSettings);
  const busy = useApp((s) => !!s.busy["settings"]);

  const [tab, setTab] = React.useState<"general" | "zcode">("general");
  const [draft, setDraft] = React.useState<SettingsType | null>(null);
  const [confirmReset, setConfirmReset] = React.useState(false);

  React.useEffect(() => {
    if (state?.settings && !draft) setDraft({ ...state.settings });
  }, [state?.settings, draft]);

  if (!state || !draft) {
    return <div className="py-24 text-center text-muted-foreground">{t("loading")}</div>;
  }

  const set = <K extends keyof SettingsType>(key: K, value: SettingsType[K]) =>
    setDraft((d) => (d ? { ...d, [key]: value } : d));

  const pickDefaultFolder = async () => {
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") set("claudeDefaultFolder", picked);
    } catch {
      /* cancelled */
    }
  };

  return (
    <>
      <PageHeader
        title={t("settingsTitle")}
        subtitle={t("settingsSubtitle")}
        actions={
          <Button size="sm" disabled={busy} onClick={() => void saveSettings(draft)}>
            {t("save")}
          </Button>
        }
      />

      <Tabs value={tab} onValueChange={(v) => setTab(v as "general" | "zcode")}>
        <TabsList>
          <TabsTrigger value="general">Genel & Sistem</TabsTrigger>
          <TabsTrigger value="zcode">ZCode Proxy & Kota</TabsTrigger>
        </TabsList>

        <TabsContent value="general">
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
                    onChange={(e) =>
                      set("language", e.target.value as SettingsType["language"])
                    }
                  >
                    <option value="tr">Türkçe</option>
                    <option value="en">English</option>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <Label>{t("defaultClaudeFolder")}</Label>
                  <div className="flex gap-2">
                    <Input
                      value={draft.claudeDefaultFolder ?? ""}
                      onChange={(e) => set("claudeDefaultFolder", e.target.value)}
                      className="font-mono text-xs"
                      placeholder="C:\\Projects"
                    />
                    <Button variant="outline" onClick={() => void pickDefaultFolder()}>
                      <FolderOpen className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* System & Tray */}
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-base">{t("settingsSystem")}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                <div className="flex items-center justify-between py-1.5">
                  <Label className="font-normal text-muted-foreground">
                    {t("autostartWindows")}
                  </Label>
                  <Switch
                    checked={draft.autostartWindows}
                    onCheckedChange={(v) => set("autostartWindows", v)}
                  />
                </div>
                <div className="flex items-center justify-between py-1.5">
                  <Label className="font-normal text-muted-foreground">
                    {t("startMinimized")}
                  </Label>
                  <Switch
                    checked={draft.startMinimized}
                    onCheckedChange={(v) => set("startMinimized", v)}
                  />
                </div>
                <div className="flex items-center justify-between py-1.5">
                  <Label className="font-normal text-muted-foreground">
                    {t("closeToTray")}
                  </Label>
                  <Switch
                    checked={draft.closeToTray}
                    onCheckedChange={(v) => set("closeToTray", v)}
                  />
                </div>
              </CardContent>
            </Card>

            {/* Paths & Versions */}
            <Card className="lg:col-span-2">
              <CardHeader className="pb-2">
                <CardTitle className="text-base">{t("settingsAbout")}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                <KeyValue label={t("appVersion")} value={state.versions.app} />
                <KeyValue
                  label={t("proxyVersion")}
                  value={state.versions.proxy ?? t("unknown")}
                />
                <SectionTitle>{t("vaultPath")}</SectionTitle>
                <code className="block break-all rounded border bg-muted/40 px-2 py-1 font-mono text-[11px]">
                  {state.paths.vault}
                </code>
                <SectionTitle>{t("dataDir")}</SectionTitle>
                <code className="block break-all rounded border bg-muted/40 px-2 py-1 font-mono text-[11px]">
                  {state.paths.dataDir}
                </code>
                <div className="flex flex-wrap gap-2 pt-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={async () => {
                      try {
                        const { revealItemInDir } = await import(
                          "@tauri-apps/plugin-opener"
                        );
                        await revealItemInDir(state.paths.dataDir);
                      } catch (e) {
                        toast.error(String(e));
                      }
                    }}
                  >
                    {t("dataDir")}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setConfirmReset(true)}
                  >
                    <RotateCcw className="h-3.5 w-3.5" />
                    {t("resetPanelToken")}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="zcode">
          <ZCodeServicePanel draft={draft} onDraftChange={set} />
        </TabsContent>
      </Tabs>

      <ConfirmDialog
        open={confirmReset}
        onClose={() => setConfirmReset(false)}
        onConfirm={async () => {
          await api.resetPanelIdentity();
          setConfirmReset(false);
          toast.success(t("saved"));
        }}
        title={t("resetPanelToken")}
        body={t("resetPanelTokenConfirm")}
      />
    </>
  );
}
