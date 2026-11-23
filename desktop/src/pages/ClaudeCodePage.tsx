import * as React from "react";
import {
  Download,
  KeyRound,
  LogIn,
  Plus,
  PowerOff,
  ShieldCheck,
  Terminal,
  Zap,
} from "lucide-react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Dialog,
  Input,
  Label,
  Select,
} from "@/components/ui";
import { EmptyState, KeyValue, PageHeader } from "@/components/common";
import {
  FolderLauncherCard,
  HarnessOAuthBanner,
  SessionCard,
} from "@/components/HarnessShared";
import { ZCodeApiKeyDialog, ZCodeOAuthDialog } from "@/components/ZCodeDialogs";
import { useApp, useT } from "@/stores/app";
import { MODEL_CATALOG } from "@/lib/i18n";
import type { ClaudeMethod, HarnessSessionSummary } from "@/lib/types";
import { cn, formatDateTime } from "@/lib/utils";

const CLAUDE_API_PRESETS = [
  { id: "anthropic", label: "Anthropic (Resmi API)", baseUrl: "", model: "claude-sonnet-4-5" },
  { id: "zai", label: "Z.AI (Anthropic Uyumlu)", baseUrl: "https://api.z.ai/api/anthropic", model: "glm-5.3" },
  { id: "bigmodel", label: "BigModel (Anthropic Uyumlu)", baseUrl: "https://open.bigmodel.cn/api/anthropic", model: "glm-5.3" },
  { id: "openrouter", label: "OpenRouter", baseUrl: "https://openrouter.ai/api", model: "anthropic/claude-sonnet-4" },
  { id: "deepseek", label: "DeepSeek (Anthropic API)", baseUrl: "https://api.deepseek.com/anthropic", model: "deepseek-chat" },
  { id: "custom", label: "Özel Sağlayıcı...", baseUrl: "", model: "" },
];

export function ClaudeCodePage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const busyMap = useApp((s) => s.busy);

  const claudeCaptureOAuth = useApp((s) => s.claudeCaptureOAuth);
  const claudeStartOAuthLogin = useApp((s) => s.claudeStartOAuthLogin);
  const claudeAddApiSession = useApp((s) => s.claudeAddApiSession);
  const claudeEditSession = useApp((s) => s.claudeEditSession);
  const claudeSetMethod = useApp((s) => s.claudeSetMethod);
  const claudeLaunchFolder = useApp((s) => s.claudeLaunchFolder);

  const live = state?.claudeLive;
  const claudeSessions = (state?.sessions ?? []).filter((s) => s.harness === "claude");
  const claudeOAuthSessions = claudeSessions.filter((s) => s.enabled && s.kind === "claude_oauth");
  const claudeApiSessions = claudeSessions.filter((s) => s.enabled && s.kind === "claude_api");
  const zcodeOAuthAccounts = (state?.accounts ?? []).filter(
    (a) => a.enabled && a.kind === "oauth",
  );
  const zApiAccounts = (state?.accounts ?? []).filter((a) => a.enabled);

  const [selectedMethod, setSelectedMethod] = React.useState<ClaudeMethod>("claude_oauth");
  const [selectedClaudeSessionId, setSelectedClaudeSessionId] = React.useState("");
  const [selectedCustomApiId, setSelectedCustomApiId] = React.useState("");
  const [selectedZcodeId, setSelectedZcodeId] = React.useState("");
  const [selectedZApiId, setSelectedZApiId] = React.useState("");
  const [model, setModel] = React.useState("glm-5.3");

  // Dialog states
  const [addOpen, setAddOpen] = React.useState(false);
  const [newName, setNewName] = React.useState("");
  const [captureOpen, setCaptureOpen] = React.useState(false);
  const [captureName, setCaptureName] = React.useState("");
  const [zcodeOAuthOpen, setZcodeOAuthOpen] = React.useState(false);
  const [zcodeApiOpen, setZcodeApiOpen] = React.useState(false);

  // Provider API dialog (create or edit)
  const [apiDialogOpen, setApiDialogOpen] = React.useState(false);
  const [editingApiSession, setEditingApiSession] = React.useState<HarnessSessionSummary | null>(
    null,
  );
  const [apiName, setApiName] = React.useState("");
  const [apiPreset, setApiPreset] = React.useState("anthropic");
  const [customProviderId, setCustomProviderId] = React.useState("anthropic");
  const [apiKey, setApiKey] = React.useState("");
  const [apiBaseUrl, setApiBaseUrl] = React.useState("");
  const [apiModel, setApiModel] = React.useState("claude-sonnet-4-5");

  React.useEffect(() => {
    if (!state) return;
    const m = state.settings.claudeActiveMethod;
    if (
      m === "claude_oauth" ||
      m === "zcode_oauth" ||
      m === "z_api" ||
      m === "custom_api" ||
      m === "disabled"
    ) {
      setSelectedMethod(m);
    }
    if (state.settings.claudeActiveModel) {
      setModel(state.settings.claudeActiveModel);
    } else if (state.settings.defaultModel) {
      setModel(state.settings.defaultModel);
    }
    if (
      state.settings.claudeActiveMethod === "claude_oauth" &&
      state.settings.claudeActiveSessionId
    ) {
      setSelectedClaudeSessionId(state.settings.claudeActiveSessionId);
    } else if (!selectedClaudeSessionId && claudeOAuthSessions[0]) {
      setSelectedClaudeSessionId(claudeOAuthSessions[0].id);
    }

    if (
      state.settings.claudeActiveMethod === "custom_api" &&
      state.settings.claudeActiveSessionId
    ) {
      setSelectedCustomApiId(state.settings.claudeActiveSessionId);
    } else if (!selectedCustomApiId && claudeApiSessions[0]) {
      setSelectedCustomApiId(claudeApiSessions[0].id);
    }

    if (
      state.settings.claudeActiveMethod === "zcode_oauth" &&
      state.settings.claudeActiveSessionId
    ) {
      setSelectedZcodeId(state.settings.claudeActiveSessionId);
    } else if (!selectedZcodeId && zcodeOAuthAccounts[0]) {
      setSelectedZcodeId(zcodeOAuthAccounts[0].id);
    }

    if (
      state.settings.claudeActiveMethod === "z_api" &&
      state.settings.claudeActiveSessionId
    ) {
      setSelectedZApiId(state.settings.claudeActiveSessionId);
    } else if (!selectedZApiId && zApiAccounts[0]) {
      setSelectedZApiId(zApiAccounts[0].id);
    }
  }, [
    state,
    claudeOAuthSessions,
    claudeApiSessions,
    zcodeOAuthAccounts,
    zApiAccounts,
    selectedClaudeSessionId,
    selectedCustomApiId,
    selectedZcodeId,
    selectedZApiId,
  ]);

  const modelOptions = Array.from(
    new Set([...MODEL_CATALOG.map((c) => c.id), ...(state?.settings.recentModels ?? []), model]),
  ).filter(Boolean);

  const openNewApiDialog = () => {
    setEditingApiSession(null);
    setApiName("");
    setApiPreset("anthropic");
    setCustomProviderId("anthropic");
    setApiKey("");
    setApiBaseUrl("");
    setApiModel("claude-sonnet-4-5");
    setApiDialogOpen(true);
  };

  const openEditApiDialog = (s: HarnessSessionSummary) => {
    setEditingApiSession(s);
    setApiName(s.name);
    const pid = typeof s.meta.providerId === "string" ? s.meta.providerId : s.providerHint ?? "custom";
    const isPreset = CLAUDE_API_PRESETS.some((p) => p.id === pid && p.id !== "custom");
    setApiPreset(isPreset ? pid : "custom");
    setCustomProviderId(pid);
    setApiKey("");
    setApiBaseUrl(typeof s.meta.baseUrl === "string" ? s.meta.baseUrl : "");
    setApiModel(typeof s.meta.model === "string" ? s.meta.model : "");
    setApiDialogOpen(true);
  };

  const applyMethod = async () => {
    if (selectedMethod === "claude_oauth") {
      await claudeSetMethod({ method: "claude_oauth", targetId: selectedClaudeSessionId });
    } else if (selectedMethod === "zcode_oauth") {
      await claudeSetMethod({ method: "zcode_oauth", targetId: selectedZcodeId, model });
    } else if (selectedMethod === "z_api") {
      await claudeSetMethod({ method: "z_api", targetId: selectedZApiId, model });
    } else if (selectedMethod === "custom_api") {
      await claudeSetMethod({ method: "custom_api", targetId: selectedCustomApiId });
    } else {
      await claudeSetMethod({ method: "disabled", clearOauthCredentials: true });
    }
  };

  const canApply =
    selectedMethod === "disabled" ||
    (selectedMethod === "claude_oauth" && !!selectedClaudeSessionId) ||
    (selectedMethod === "zcode_oauth" && !!selectedZcodeId) ||
    (selectedMethod === "z_api" && !!selectedZApiId) ||
    (selectedMethod === "custom_api" && !!selectedCustomApiId);

  const methodCards: Array<{
    id: ClaudeMethod;
    icon: React.ReactNode;
    title: string;
    desc: string;
  }> = [
    {
      id: "claude_oauth",
      icon: <ShieldCheck className="h-4 w-4 text-primary" />,
      title: t("methodClaudeOAuth"),
      desc: t("methodClaudeOAuthDesc"),
    },
    {
      id: "zcode_oauth",
      icon: <Zap className="h-4 w-4 text-primary" />,
      title: t("methodZcodeOAuth"),
      desc: t("methodZcodeOAuthDesc"),
    },
    {
      id: "z_api",
      icon: <KeyRound className="h-4 w-4 text-warning" />,
      title: t("methodZApi"),
      desc: t("methodZApiDesc"),
    },
    {
      id: "custom_api",
      icon: <KeyRound className="h-4 w-4 text-primary" />,
      title: t("methodCustomApi"),
      desc: t("methodCustomApiDesc"),
    },
    {
      id: "disabled",
      icon: <PowerOff className="h-4 w-4 text-destructive" />,
      title: t("methodDisabled"),
      desc: t("methodDisabledDesc"),
    },
  ];

  const liveBadgeVariant =
    live?.detectedMode === "disabled"
      ? "secondary"
      : live?.detectedMode === "custom_api"
        ? "warning"
        : "success";

  const oauthRunning = state?.proxy.oauthRunning ?? false;

  return (
    <>
      <PageHeader
        title="Claude Code"
        subtitle={t("claudeSubtitle")}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              disabled={!live?.hasCredentialsFile || !!busyMap["claudeCapture"]}
              onClick={() => setCaptureOpen(true)}
            >
              <Download /> {t("captureCurrentBtn")}
            </Button>
            <Button variant="outline" size="sm" onClick={openNewApiDialog}>
              <KeyRound /> Sağlayıcı API Ekle
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={oauthRunning}
              onClick={() => setZcodeOAuthOpen(true)}
            >
              <Plus /> ZCode OAuth
            </Button>
            <Button variant="outline" size="sm" onClick={() => setZcodeApiOpen(true)}>
              <Plus /> Z API
            </Button>
            <Button
              size="sm"
              disabled={state?.harnessOauthRunning || !live?.cli.found}
              onClick={() => setAddOpen(true)}
            >
              <LogIn /> Claude OAuth Ekle
            </Button>
          </>
        }
      />

      <HarnessOAuthBanner target="claude" />

      {/* Live disk state */}
      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center justify-between gap-2 text-base">
            <span className="flex items-center gap-2">
              <Terminal className="h-4 w-4 text-muted-foreground" />
              Canlı Durum (~/.claude)
            </span>
            <div className="flex items-center gap-2">
              <Badge variant={live?.cli.found ? "outline" : "warning"}>
                {live?.cli.found ? live.cli.version ?? t("cliInstalled") : t("claudeNotDetected")}
              </Badge>
              <Badge variant={liveBadgeVariant}>
                {live?.detectedMode === "claude_oauth"
                  ? t("methodClaudeOAuth")
                  : live?.detectedMode === "zcode_oauth"
                    ? t("methodZcodeOAuth")
                    : live?.detectedMode === "z_api"
                      ? t("methodZApi")
                      : live?.detectedMode === "custom_api"
                        ? t("methodCustomApi")
                        : t("disabledLabel")}
              </Badge>
            </div>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-x-8 md:grid-cols-2">
            <div>
              <KeyValue label={t("account")} value={live?.liveAccountHint ?? t("none")} mono />
              <KeyValue
                label={t("claudePath")}
                value={live?.cli.path ?? t("claudeInstallHint")}
                mono
              />
            </div>
            <div>
              <KeyValue
                label="Endpoint"
                value={live?.liveBaseUrl ?? "Anthropic OAuth / Resmi API"}
                mono
              />
              <KeyValue label={t("model")} value={live?.liveModel ?? "Varsayılan"} mono />
              {live?.liveExpiresAt && (
                <KeyValue label={t("quotaExpires")} value={formatDateTime(live.liveExpiresAt)} />
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Active Method Switcher */}
      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="text-base">{t("activeMethodLabel")}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-5">
            {methodCards.map((m) => (
              <button
                key={m.id}
                type="button"
                onClick={() => setSelectedMethod(m.id)}
                className={cn(
                  "rounded-lg border p-3.5 text-left transition-colors",
                  selectedMethod === m.id
                    ? "border-primary bg-primary/5 ring-1 ring-primary"
                    : "bg-card hover:bg-accent/50",
                )}
              >
                <div className="mb-1 flex items-center gap-2 text-sm font-medium">
                  {m.icon}
                  <span>{m.title}</span>
                </div>
                <p className="text-xs text-muted-foreground">{m.desc}</p>
              </button>
            ))}
          </div>

          {selectedMethod === "claude_oauth" && (
            <div className="space-y-1.5">
              <Label>Claude OAuth Oturumu</Label>
              {claudeOAuthSessions.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  Kayıtlı etkin Claude OAuth oturumu yok. Yukarıdan mevcut oturumu içe aktarın veya
                  yeni giriş yapın.
                </p>
              ) : (
                <Select
                  value={selectedClaudeSessionId}
                  onChange={(e) => setSelectedClaudeSessionId(e.target.value)}
                >
                  {claudeOAuthSessions.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} {s.accountHint ? `(${s.accountHint})` : ""}
                    </option>
                  ))}
                </Select>
              )}
            </div>
          )}

          {selectedMethod === "zcode_oauth" && (
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <Label>ZCode OAuth Hesabı</Label>
                  <button
                    type="button"
                    onClick={() => setZcodeOAuthOpen(true)}
                    className="text-xs text-primary hover:underline"
                  >
                    + Yeni ZCode OAuth Ekle
                  </button>
                </div>
                {zcodeOAuthAccounts.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    Etkin ZCode OAuth hesabı yok. Yukarıdan + ZCode OAuth ile ekleyebilirsiniz.
                  </p>
                ) : (
                  <Select
                    value={selectedZcodeId}
                    onChange={(e) => setSelectedZcodeId(e.target.value)}
                  >
                    {zcodeOAuthAccounts.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.name} ({a.provider.toUpperCase()})
                      </option>
                    ))}
                  </Select>
                )}
              </div>
              <div className="space-y-1.5">
                <Label>{t("selectModel")}</Label>
                <Select
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
          )}

          {selectedMethod === "z_api" && (
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <Label>Z API / Z.AI Hesabı</Label>
                  <button
                    type="button"
                    onClick={() => setZcodeApiOpen(true)}
                    className="text-xs text-primary hover:underline"
                  >
                    + Yeni Z API Ekle
                  </button>
                </div>
                {zApiAccounts.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    Etkin Z.AI / BigModel hesabı yok. Yukarıdan + Z API ile ekleyebilirsiniz.
                  </p>
                ) : (
                  <Select
                    value={selectedZApiId}
                    onChange={(e) => setSelectedZApiId(e.target.value)}
                  >
                    {zApiAccounts.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.name} ({a.provider.toUpperCase()} ·{" "}
                        {a.kind === "api-key" ? "API Key" : "OAuth Key"})
                      </option>
                    ))}
                  </Select>
                )}
              </div>
              <div className="space-y-1.5">
                <Label>{t("selectModel")}</Label>
                <Select
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
          )}

          {selectedMethod === "custom_api" && (
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <Label>Kayıtlı Sağlayıcı API Hesabı</Label>
                <button
                  type="button"
                  onClick={openNewApiDialog}
                  className="text-xs text-primary hover:underline"
                >
                  + Yeni Sağlayıcı API Ekle
                </button>
              </div>
              {claudeApiSessions.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  Kayıtlı Sağlayıcı API oturumu yok. Yukarıdan ekleyin.
                </p>
              ) : (
                <Select
                  value={selectedCustomApiId}
                  onChange={(e) => setSelectedCustomApiId(e.target.value)}
                >
                  {claudeApiSessions.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.providerHint ?? "custom"} · {s.accountHint ?? ""})
                    </option>
                  ))}
                </Select>
              )}
            </div>
          )}

          <div className="flex items-center justify-between gap-4 pt-1">
            <p className="text-xs text-muted-foreground">
              Seçilen yöntem global `~/.claude/settings.json` ve kimlik dosyalarına yazılır; her
              terminalden doğrudan `claude` komutuyla çalışır.
            </p>
            <Button
              disabled={!canApply || !!busyMap["claudeMethod"]}
              variant={selectedMethod === "disabled" ? "destructive" : "default"}
              onClick={() => void applyMethod()}
            >
              {selectedMethod === "disabled" ? t("disableBtn") : t("activate")}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Saved Claude Code Sessions (OAuth + Provider API) */}
      <div className="mb-4">
        <h2 className="mb-3 text-base font-semibold">Kayıtlı Claude Code Oturumları</h2>
        {claudeSessions.length === 0 ? (
          <EmptyState
            icon={<ShieldCheck className="h-10 w-10" />}
            title="Henüz Claude Code oturumu yok"
            description="Claude OAuth, mevcut oturum içe aktarma veya dilediğiniz sağlayıcı API anahtarını ekleyin."
          />
        ) : (
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
            {claudeSessions.map((s) => (
              <SessionCard
                key={s.id}
                session={s}
                kindLabel={s.kind === "claude_api" ? "Sağlayıcı API" : "Claude OAuth"}
                extraRows={[
                  ...(typeof s.meta.model === "string" && s.meta.model
                    ? [{ label: t("model"), value: s.meta.model, mono: true }]
                    : []),
                  ...(typeof s.meta.baseUrl === "string" && s.meta.baseUrl
                    ? [{ label: "Base URL", value: s.meta.baseUrl, mono: true }]
                    : []),
                  ...(s.expiresAt
                    ? [{ label: t("quotaExpires"), value: formatDateTime(s.expiresAt) }]
                    : []),
                ]}
                activateBusy={!!busyMap["claudeMethod"]}
                onEdit={s.kind === "claude_api" ? () => openEditApiDialog(s) : undefined}
                onActivate={() =>
                  claudeSetMethod({
                    method: s.kind === "claude_api" ? "custom_api" : "claude_oauth",
                    targetId: s.id,
                  })
                }
              />
            ))}
          </div>
        )}
      </div>

      <FolderLauncherCard
        title="Claude Code Başlatıcı"
        disabled={!live?.cli.found}
        busy={!!busyMap["claudeLaunch"]}
        onLaunch={(folder) => claudeLaunchFolder(folder)}
      />

      {/* Capture Dialog */}
      <Dialog
        open={captureOpen}
        onClose={() => setCaptureOpen(false)}
        title={t("captureCurrentBtn")}
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. Kişisel Claude Max"
              value={captureName}
              onChange={(e) => setCaptureName(e.target.value)}
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setCaptureOpen(false)}>
              {t("cancel")}
            </Button>
            <Button
              onClick={async () => {
                await claudeCaptureOAuth(captureName.trim() || undefined);
                setCaptureName("");
                setCaptureOpen(false);
              }}
            >
              {t("save")}
            </Button>
          </div>
        </div>
      </Dialog>

      {/* OAuth Login Dialog */}
      <Dialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title="Yeni Claude OAuth Oturumu"
        description="İzole bir terminal penceresinde `claude auth login` açılır; mevcut oturumunuz etkilenmez."
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. İş Claude Hesabım"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setAddOpen(false)}>
              {t("cancel")}
            </Button>
            <Button
              onClick={async () => {
                setAddOpen(false);
                await claudeStartOAuthLogin(newName.trim() || undefined);
                setNewName("");
              }}
            >
              <LogIn /> {t("launch")}
            </Button>
          </div>
        </div>
      </Dialog>

      {/* Provider API Create / Edit Dialog */}
      <Dialog
        open={apiDialogOpen}
        onClose={() => setApiDialogOpen(false)}
        title={editingApiSession ? "Claude Code API Oturumunu Düzenle" : "Claude Code Sağlayıcı API Ekle"}
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. Anthropic Work / OpenRouter"
              value={apiName}
              onChange={(e) => setApiName(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label>{t("provider")}</Label>
            <Select
              value={apiPreset}
              onChange={(e) => {
                const p = e.target.value;
                setApiPreset(p);
                if (p !== "custom") {
                  setCustomProviderId(p);
                  const found = CLAUDE_API_PRESETS.find((x) => x.id === p);
                  if (found) {
                    setApiBaseUrl(found.baseUrl);
                    if (found.model) setApiModel(found.model);
                  }
                }
              }}
            >
              {CLAUDE_API_PRESETS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </Select>
          </div>
          {apiPreset === "custom" && (
            <div className="space-y-1.5">
              <Label>Özel Sağlayıcı Kimliği</Label>
              <Input
                value={customProviderId}
                onChange={(e) => setCustomProviderId(e.target.value)}
                placeholder="örn. litellm, minimax, together"
                className="font-mono"
              />
            </div>
          )}
          <div className="space-y-1.5">
            <Label>
              {t("apiKeyLabel")} {editingApiSession ? "(boş bırakılırsa mevcut korunur)" : ""}
            </Label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={editingApiSession ? "Değiştirmek için yeni anahtar girin" : "sk-..."}
              className="font-mono"
            />
          </div>
          <div className="space-y-1.5">
            <Label>{t("baseUrlOptional")}</Label>
            <Input
              value={apiBaseUrl}
              onChange={(e) => setApiBaseUrl(e.target.value)}
              placeholder="Boş = Resmi Anthropic API (ANTHROPIC_API_KEY)"
              className="font-mono"
            />
          </div>
          <div className="space-y-1.5">
            <Label>Model (opsiyonel)</Label>
            <Input
              value={apiModel}
              onChange={(e) => setApiModel(e.target.value)}
              placeholder="claude-sonnet-4-5"
              className="font-mono"
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setApiDialogOpen(false)}>
              {t("cancel")}
            </Button>
            <Button
              disabled={
                !customProviderId.trim() ||
                (!editingApiSession && apiKey.trim().length < 4)
              }
              onClick={async () => {
                if (editingApiSession) {
                  await claudeEditSession({
                    id: editingApiSession.id,
                    name: apiName.trim() || undefined,
                    providerId: customProviderId.trim(),
                    apiKey: apiKey.trim() || undefined,
                    baseUrl: apiBaseUrl.trim(),
                    model: apiModel.trim(),
                  });
                } else {
                  await claudeAddApiSession({
                    name: apiName,
                    providerId: customProviderId.trim(),
                    apiKey,
                    baseUrl: apiBaseUrl.trim() || undefined,
                    model: apiModel.trim() || undefined,
                  });
                }
                setApiDialogOpen(false);
              }}
            >
              {t("save")}
            </Button>
          </div>
        </div>
      </Dialog>

      <ZCodeOAuthDialog open={zcodeOAuthOpen} onClose={() => setZcodeOAuthOpen(false)} />
      <ZCodeApiKeyDialog open={zcodeApiOpen} onClose={() => setZcodeApiOpen(false)} />
    </>
  );
}
