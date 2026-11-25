import * as React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  Code2,
  Cpu,
  Download,
  FolderOpen,
  Pencil,
  Play,
  Plus,
  PowerOff,
  Sparkles,
  Terminal,
} from "lucide-react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Select,
  Switch,
} from "@/components/ui";
import { PageHeader } from "@/components/common";
import { cn } from "@/lib/utils";
import { useApp, useT } from "@/stores/app";
import type {
  AccountKind,
  HarnessId,
  UnifiedAccountSummary,
} from "@/lib/types";

const KIND_LABEL: Record<AccountKind, string> = {
  claude_oauth: "Claude OAuth",
  custom_provider: "Özel Model / API",
  z_api: "Z.AI API",
  zcode_oauth: "ZCode OAuth",
  antigravity_oauth: "Antigravity",
};

function modelsForAccount(acc: UnifiedAccountSummary | undefined): string[] {
  if (!acc) return [];
  if (acc.kind === "custom_provider") {
    const ids = acc.models.map((m) => m.id);
    if (acc.defaultModel && !ids.includes(acc.defaultModel)) ids.unshift(acc.defaultModel);
    return ids;
  }
  if (acc.kind === "zcode_oauth" || acc.kind === "z_api") {
    return Array.from(
      new Set([
        ...(acc.defaultModel ? [acc.defaultModel] : []),
        "glm-5.3",
        "glm-5.3-flash",
        "glm-5.2",
        "glm-5-turbo",
        "glm-4.7",
      ]),
    );
  }
  return [];
}

function slugForOpenCode(acc: UnifiedAccountSummary): string {
  if (acc.kind === "z_api") return `zai-api-${acc.id.slice(0, 6)}`;
  if (acc.kind === "zcode_oauth") return `zcode-oauth-${acc.id.slice(0, 6)}`;
  return acc.provider;
}

function HarnessQuickCard({
  harness,
  title,
  subtitle,
  icon,
  defaultAddKind,
  cliBadge,
  statusBadge,
  statusHint,
  compatibleAccounts,
  activeAccountId,
  activeModel,
  multiActiveMode,
  canLaunchFolder,
  extraAction,
}: {
  harness: HarnessId;
  title: string;
  subtitle: string;
  icon: React.ReactNode;
  defaultAddKind: AccountKind;
  cliBadge: React.ReactNode;
  statusBadge: React.ReactNode;
  statusHint?: string | null;
  compatibleAccounts: UnifiedAccountSummary[];
  activeAccountId: string | null;
  activeModel: string | null;
  multiActiveMode?: boolean;
  canLaunchFolder?: boolean;
  extraAction?: React.ReactNode;
}) {
  const t = useT();
  const state = useApp((s) => s.state)!;
  const openAddModalFor = useApp((s) => s.openAddModalFor);
  const openEditModalFor = useApp((s) => s.openEditModalFor);
  const activateForHarness = useApp((s) => s.activateForHarness);
  const opencodeToggleAccount = useApp((s) => s.opencodeToggleAccount);
  const opencodeSetDefaultAccount = useApp((s) => s.opencodeSetDefaultAccount);
  const disableHarness = useApp((s) => s.disableHarness);
  const launchHarness = useApp((s) => s.launchHarness);
  const busy = useApp(
    (s) => !!s.busy[`harness:${harness}`] || !!s.busy[`launch:${harness}`],
  );

  const pickAndLaunch = async () => {
    if (!canLaunchFolder) {
      await launchHarness(harness);
      return;
    }
    const defaultFolder =
      state.settings.claudeDefaultFolder ??
      state.settings.recentFolders[0] ??
      undefined;
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        defaultPath: defaultFolder,
      });
      if (typeof picked === "string") {
        await launchHarness(harness, picked);
      }
    } catch {
      /* cancelled */
    }
  };

  return (
    <Card className="flex flex-col justify-between">
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2.5">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
              {icon}
            </div>
            <div>
              <CardTitle className="text-base">{title}</CardTitle>
              <CardDescription className="text-xs">{subtitle}</CardDescription>
            </div>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-1.5">
            {cliBadge}
            {statusBadge}
          </div>
        </div>
        {statusHint && (
          <div className="mt-2 truncate rounded bg-muted/40 px-2.5 py-1 font-mono text-[11px] text-muted-foreground">
            {statusHint}
          </div>
        )}
      </CardHeader>

      <CardContent className="space-y-3">
        {compatibleAccounts.length === 0 ? (
          <div className="flex items-center justify-between rounded-md border border-dashed px-3 py-3 text-xs text-muted-foreground">
            <span>{t("noCompatibleAccounts")}</span>
            <Button
              size="sm"
              variant="outline"
              onClick={() => openAddModalFor(defaultAddKind)}
            >
              <Plus className="h-3.5 w-3.5" />
              {t("addAccountCta")}
            </Button>
          </div>
        ) : (
          <div className="max-h-64 space-y-1.5 overflow-y-auto pr-0.5">
            {compatibleAccounts.map((acc) => {
              const ocSlug = slugForOpenCode(acc);
              const inOpenCode =
                !!multiActiveMode &&
                state.opencodeLive.providers.some((p) => p.id === ocSlug);
              const isDefaultOpenCode =
                multiActiveMode &&
                !!state.opencodeLive.activeModel &&
                state.opencodeLive.activeModel.startsWith(`${ocSlug}/`);
              const isActive = multiActiveMode ? inOpenCode : activeAccountId === acc.id;

              const models = modelsForAccount(acc);
              const currentModel = isDefaultOpenCode && state.opencodeLive.activeModel
                ? state.opencodeLive.activeModel.slice(ocSlug.length + 1)
                : !multiActiveMode && isActive && activeModel
                  ? activeModel
                  : acc.defaultModel ?? models[0] ?? undefined;
              const hint = acc.userHint || acc.keyHint || acc.baseUrl || acc.provider;

              return (
                <div
                  key={acc.id}
                  className={cn(
                    "flex flex-wrap items-center justify-between gap-2 rounded-md border px-2.5 py-2 transition-colors",
                    isActive
                      ? "border-primary/60 bg-primary/10"
                      : "border-border bg-background hover:bg-accent/50",
                  )}
                >
                  {multiActiveMode ? (
                    <div className="flex min-w-0 flex-1 items-center gap-2.5">
                      <Switch
                        checked={inOpenCode}
                        disabled={busy}
                        onCheckedChange={(checked) =>
                          void opencodeToggleAccount(acc.id, checked, currentModel)
                        }
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className="truncate text-sm font-medium">{acc.name}</span>
                          <Badge variant="secondary" className="text-[10px]">
                            {KIND_LABEL[acc.kind]}
                          </Badge>
                        </div>
                        {hint && (
                          <div className="truncate font-mono text-[11px] text-muted-foreground">
                            {hint}
                          </div>
                        )}
                      </div>
                    </div>
                  ) : (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => {
                        if (!isActive) {
                          void activateForHarness(harness, acc.id, currentModel);
                        }
                      }}
                      className="flex min-w-0 flex-1 items-center gap-2 text-left disabled:opacity-60"
                    >
                      <CheckCircle2
                        className={cn(
                          "h-4 w-4 shrink-0",
                          isActive ? "text-primary" : "text-muted-foreground/30",
                        )}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className="truncate text-sm font-medium">{acc.name}</span>
                          <Badge variant="secondary" className="text-[10px]">
                            {KIND_LABEL[acc.kind]}
                          </Badge>
                        </div>
                        {hint && (
                          <div className="truncate font-mono text-[11px] text-muted-foreground">
                            {hint}
                          </div>
                        )}
                      </div>
                    </button>
                  )}

                  <div className="flex shrink-0 items-center gap-1">
                    {isActive && models.length > 1 && (
                      <Select
                        className="h-7 w-32 px-2 text-xs font-mono"
                        value={currentModel ?? ""}
                        disabled={busy}
                        onChange={(e) =>
                          multiActiveMode
                            ? void opencodeSetDefaultAccount(acc.id, e.target.value)
                            : void activateForHarness(harness, acc.id, e.target.value)
                        }
                      >
                        {models.map((m) => (
                          <option key={m} value={m}>
                            {m}
                          </option>
                        ))}
                      </Select>
                    )}

                    {multiActiveMode ? (
                      isDefaultOpenCode ? (
                        <Badge variant="success" className="text-[10px]">
                          Varsayılan
                        </Badge>
                      ) : (
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-7 px-2 text-xs"
                          disabled={busy}
                          onClick={() =>
                            void opencodeSetDefaultAccount(acc.id, currentModel)
                          }
                        >
                          Varsayılan Yap
                        </Button>
                      )
                    ) : isActive ? (
                      <Badge variant="success" className="text-[10px]">
                        {t("active")}
                      </Badge>
                    ) : (
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-7 px-2 text-xs"
                        disabled={busy}
                        onClick={() =>
                          void activateForHarness(harness, acc.id, currentModel)
                        }
                      >
                        Geçiş Yap
                      </Button>
                    )}

                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 w-7 p-0"
                      onClick={() => openEditModalFor(acc.id)}
                      title={t("edit")}
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        <div className="flex flex-wrap items-center justify-between gap-2 border-t pt-2.5">
          <div className="flex flex-wrap items-center gap-1.5">
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => openAddModalFor(defaultAddKind)}
            >
              <Plus className="h-3.5 w-3.5" />
              {t("add")}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => void disableHarness(harness)}
              title={t("disable")}
            >
              <PowerOff className="h-3.5 w-3.5" />
              {t("disable")}
            </Button>
            {extraAction}
          </div>

          <Button
            size="sm"
            variant="secondary"
            disabled={busy}
            onClick={() => void pickAndLaunch()}
          >
            {canLaunchFolder ? (
              <FolderOpen className="h-3.5 w-3.5" />
            ) : (
              <Play className="h-3.5 w-3.5" />
            )}
            {canLaunchFolder ? t("launch") : t("zcodeLaunchApp")}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

export function HarnessesPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const importSystemAccounts = useApp((s) => s.importSystemAccounts);
  const captureAgyOAuth = useApp((s) => s.captureAgyOAuth);
  const busy = useApp((s) => s.busy);

  if (!state) {
    return <div className="py-24 text-center text-muted-foreground">{t("loading")}</div>;
  }

  const enabledAccounts = state.accounts.filter((a) => a.enabled);
  const compat = (h: HarnessId) =>
    enabledAccounts.filter((a) => a.supportedHarnesses.includes(h));

  const effectiveClaudeActiveId =
    state.settings.claudeActiveSessionId ??
    (state.claudeLive.detectedMode === "claude_oauth"
      ? compat("claude").find(
          (a) =>
            a.kind === "claude_oauth" &&
            (!state.claudeLive.liveAccountHint ||
              a.userHint === state.claudeLive.liveAccountHint),
        )?.id ??
        compat("claude").find((a) => a.kind === "claude_oauth")?.id ??
        null
      : null);

  const effectiveOpenCodeActiveId =
    state.settings.opencodeActiveSessionId ??
    (state.opencodeLive.activeModel
      ? compat("opencode").find((a) =>
          state.opencodeLive.activeModel!.startsWith(`${slugForOpenCode(a)}/`),
        )?.id ?? null
      : null);

  const effectiveZcodeActiveId =
    state.settings.zcodeAppActiveAccountId ??
    (state.zcodeLive.zaiMode === "api-key" && state.zcodeLive.activePersonalName
      ? compat("zcode").find(
          (a) =>
            a.kind === "z_api" &&
            (a.name === state.zcodeLive.activePersonalName ||
              a.name.startsWith(state.zcodeLive.activePersonalName!)),
        )?.id ?? null
      : state.zcodeLive.v2Exists && state.zcodeLive.zaiMode === "oauth"
        ? compat("zcode").find((a) => a.kind === "zcode_oauth")?.id ?? null
        : null);

  const effectiveAgyActiveId =
    state.settings.antigravityActiveSessionId ??
    (state.antigravityLive.hasKeyringToken || state.antigravityLive.hasFileToken
      ? compat("antigravity").find(
          (a) =>
            a.kind === "antigravity_oauth" &&
            ((state.antigravityLive.activeEmail &&
              a.userHint === state.antigravityLive.activeEmail) ||
              (state.antigravityLive.tokenHint &&
                a.keyHint === state.antigravityLive.tokenHint)),
        )?.id ??
        compat("antigravity").find((a) => a.kind === "antigravity_oauth")?.id ??
        null
      : null);

  const unimportedOpenCodeProviders = state.opencodeLive.providers.filter(
    (p) =>
      !p.isManaged &&
      !!p.baseUrl &&
      !state.accounts.some(
        (a) =>
          a.kind === "custom_provider" &&
          (a.provider === p.id || a.baseUrl === p.baseUrl),
      ),
  );

  return (
    <>
      <PageHeader
        title={t("harnessesTitle")}
        subtitle={t("harnessesSubtitle")}
        actions={
          <Button
            size="sm"
            variant="outline"
            disabled={!!busy["import"]}
            onClick={() => void importSystemAccounts()}
          >
            <Download className="h-4 w-4" />
            {t("importSystem")}
          </Button>
        }
      />

      {unimportedOpenCodeProviders.length > 0 && (
        <div className="mb-4 flex flex-wrap items-center justify-between gap-2 rounded-lg border border-primary/40 bg-primary/5 px-4 py-2.5 text-xs">
          <span>
            Sistemdeki <code className="font-mono">opencode.jsonc</code> dosyasında kasaya eklenmemiş{" "}
            <strong>{unimportedOpenCodeProviders.length}</strong> model/sağlayıcı bulundu (
            {unimportedOpenCodeProviders.map((p) => p.name).join(", ")}).
          </span>
          <Button
            size="sm"
            variant="default"
            disabled={!!busy["import"]}
            onClick={() => void importSystemAccounts()}
          >
            <Download className="h-3.5 w-3.5" />
            Kasaya İçe Aktar
          </Button>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* Claude Code */}
        <HarnessQuickCard
          harness="claude"
          title="Claude Code"
          subtitle="Tek aktif yöntem — OAuth, Z.AI API, ZCode Proxy & Özel Model (settings.json)"
          icon={<Terminal className="h-5 w-5" />}
          defaultAddKind="claude_oauth"
          cliBadge={
            <Badge variant={state.claudeLive.cli.found ? "success" : "secondary"}>
              {state.claudeLive.cli.found
                ? `CLI ${state.claudeLive.cli.version ?? "OK"}`
                : t("cliMissing")}
            </Badge>
          }
          statusBadge={
            <Badge
              variant={
                state.claudeLive.detectedMode === "disabled" ? "outline" : "default"
              }
            >
              {state.claudeLive.detectedMode}
            </Badge>
          }
          statusHint={
            state.claudeLive.liveModel ??
            state.claudeLive.liveAccountHint ??
            state.claudeLive.liveBaseUrl
          }
          compatibleAccounts={compat("claude")}
          activeAccountId={effectiveClaudeActiveId}
          activeModel={state.settings.claudeActiveModel}
          canLaunchFolder
        />

        {/* OpenCode (multi-active providers) */}
        <HarnessQuickCard
          harness="opencode"
          title="OpenCode"
          subtitle="Aynı anda birden fazla model & sağlayıcı aktif edilebilir (opencode.jsonc)"
          icon={<Code2 className="h-5 w-5" />}
          defaultAddKind="custom_provider"
          multiActiveMode
          cliBadge={
            <Badge variant={state.opencodeLive.cli.found ? "success" : "secondary"}>
              {state.opencodeLive.cli.found
                ? `CLI ${state.opencodeLive.cli.version ?? "OK"}`
                : t("cliMissing")}
            </Badge>
          }
          statusBadge={
            <Badge
              variant={state.opencodeLive.providers.length === 0 ? "outline" : "success"}
            >
              {state.opencodeLive.providers.length} sağlayıcı aktif
            </Badge>
          }
          statusHint={
            state.opencodeLive.activeModel ??
            state.opencodeLive.baseUrl ??
            state.opencodeLive.configPath
          }
          compatibleAccounts={compat("opencode")}
          activeAccountId={effectiveOpenCodeActiveId}
          activeModel={state.settings.opencodeActiveModel}
          canLaunchFolder
        />

        {/* ZCode Desktop */}
        <HarnessQuickCard
          harness="zcode"
          title="ZCode Desktop"
          subtitle="~/.zcode/v2 — ZCode OAuth & Z.AI API Anahtarı geçişi"
          icon={<Cpu className="h-5 w-5" />}
          defaultAddKind="z_api"
          cliBadge={
            <Badge variant={state.zcodeLive.installed ? "success" : "secondary"}>
              {state.zcodeLive.installed ? t("cliInstalled") : t("cliMissing")}
            </Badge>
          }
          statusBadge={
            <Badge
              variant={state.zcodeLive.zaiMode === "api-key" ? "default" : "success"}
            >
              {state.zcodeLive.zaiMode === "oauth"
                ? "OAuth (Canlı)"
                : state.zcodeLive.zaiMode === "api-key"
                  ? "API Key (Canlı)"
                  : state.zcodeLive.zaiMode}
            </Badge>
          }
          statusHint={
            state.zcodeLive.activePersonalName ?? state.zcodeLive.selectedKey
          }
          compatibleAccounts={compat("zcode")}
          activeAccountId={effectiveZcodeActiveId}
          activeModel={null}
        />

        {/* Antigravity CLI */}
        <HarnessQuickCard
          harness="antigravity"
          title="Antigravity CLI (agy)"
          subtitle="Google OAuth oturum değiştirici"
          icon={<Sparkles className="h-5 w-5" />}
          defaultAddKind="antigravity_oauth"
          cliBadge={
            <Badge variant={state.antigravityLive.cli.found ? "success" : "secondary"}>
              {state.antigravityLive.cli.found ? t("cliInstalled") : t("cliMissing")}
            </Badge>
          }
          statusBadge={
            <Badge
              variant={
                state.antigravityLive.hasKeyringToken || state.antigravityLive.hasFileToken
                  ? state.antigravityLive.isExpired
                    ? "warning"
                    : "success"
                  : "outline"
              }
            >
              {state.antigravityLive.hasKeyringToken || state.antigravityLive.hasFileToken
                ? "Oturum Aktif"
                : t("disabled")}
            </Badge>
          }
          statusHint={
            state.antigravityLive.activeEmail ?? state.antigravityLive.tokenHint
          }
          compatibleAccounts={compat("antigravity")}
          activeAccountId={effectiveAgyActiveId}
          activeModel={null}
          canLaunchFolder
          extraAction={
            <Button
              size="sm"
              variant="ghost"
              disabled={!!busy["captureAgy"]}
              onClick={() => void captureAgyOAuth()}
            >
              {t("agyCaptureBtn")}
            </Button>
          }
        />
      </div>
    </>
  );
}
