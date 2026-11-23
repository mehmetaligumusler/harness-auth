import * as React from "react";
import {
  Code2,
  Download,
  KeyRound,
  LogIn,
  Plus,
  PowerOff,
  ShieldCheck,
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
  DisableHarnessButton,
  FolderLauncherCard,
  HarnessOAuthBanner,
  SessionCard,
} from "@/components/HarnessShared";
import { ZCodeApiKeyDialog, ZCodeOAuthDialog } from "@/components/ZCodeDialogs";
import { useApp, useT } from "@/stores/app";
import { MODEL_CATALOG } from "@/lib/i18n";
import type { HarnessSessionSummary, OpenCodeMethod } from "@/lib/types";
import { cn } from "@/lib/utils";

const PRESET_PROVIDERS = [
  { id: "anthropic", label: "Anthropic", baseUrl: "" },
  { id: "openai", label: "OpenAI", baseUrl: "" },
  { id: "google", label: "Google Gemini", baseUrl: "" },
  { id: "zai", label: "Z.AI (Anthropic API)", baseUrl: "https://api.z.ai/api/anthropic" },
  { id: "bigmodel", label: "BigModel (OpenAI API)", baseUrl: "https://open.bigmodel.cn/api/paas/v4" },
  { id: "openrouter", label: "OpenRouter", baseUrl: "https://openrouter.ai/api/v1" },
  { id: "custom", label: "Özel Sağlayıcı...", baseUrl: "" },
];

export function OpenCodePage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const busyMap = useApp((s) => s.busy);

  const captureCurrent = useApp((s) => s.opencodeCaptureCurrent);
  const addApiSession = useApp((s) => s.opencodeAddApiSession);
  const addZcodeSession = useApp((s) => s.opencodeAddZcodeSession);
  const startOAuth = useApp((s) => s.opencodeStartOAuthLogin);
  const activateSession = useApp((s) => s.opencodeActivateSession);
  const setMethod = useApp((s) => s.opencodeSetMethod);
  const editSession = useApp((s) => s.opencodeEditSession);
  const disable = useApp((s) => s.opencodeDisable);
  const launchFolder = useApp((s) => s.opencodeLaunchFolder);

  const live = state?.opencodeLive;
  const sessions = (state?.sessions ?? []).filter((s) => s.harness === "opencode");
  const enabledSessions = sessions.filter((s) => s.enabled);
  const zcodeOAuthAccounts = (state?.accounts ?? []).filter(
    (a) => a.enabled && a.kind === "oauth",
  );
  const zApiAccounts = (state?.accounts ?? []).filter((a) => a.enabled);
  const zcodeAccounts = (state?.accounts ?? []).filter((a) => a.enabled);

  // Active method switcher state
  const [selectedMethod, setSelectedMethod] = React.useState<OpenCodeMethod>("profile");
  const [selectedProfileId, setSelectedProfileId] = React.useState("");
  const [selectedZcodeId, setSelectedZcodeId] = React.useState("");
  const [selectedZApiId, setSelectedZApiId] = React.useState("");
  const [model, setModel] = React.useState("glm-5.3");

  // Dialog states
  const [captureOpen, setCaptureOpen] = React.useState(false);
  const [captureName, setCaptureName] = React.useState("");
  const [zcodeOAuthOpen, setZcodeOAuthOpen] = React.useState(false);
  const [zcodeApiOpen, setZcodeApiOpen] = React.useState(false);

  // API create/edit dialog
  const [apiOpen, setApiOpen] = React.useState(false);
  const [editingSession, setEditingSession] = React.useState<HarnessSessionSummary | null>(null);
  const [apiName, setApiName] = React.useState("");
  const [providerPreset, setProviderPreset] = React.useState("anthropic");
  const [customProviderId, setCustomProviderId] = React.useState("anthropic");
  const [apiKey, setApiKey] = React.useState("");
  const [baseUrl, setBaseUrl] = React.useState("");
  const [apiModel, setApiModel] = React.useState("");

  // ZCode bridge profile dialog (create or edit)
  const [zcodeOpen, setZcodeOpen] = React.useState(false);
  const [editingZcodeBridge, setEditingZcodeBridge] = React.useState<HarnessSessionSummary | null>(
    null,
  );
  const [zcodeName, setZcodeName] = React.useState("");
  const [zcodeAccId, setZcodeAccId] = React.useState("");
  const [zcodeModel, setZcodeModel] = React.useState("glm-5.3");

  const [oauthOpen, setOauthOpen] = React.useState(false);
  const [oauthName, setOauthName] = React.useState("");

  React.useEffect(() => {
    if (!state) return;
    const m = state.settings.opencodeActiveMethod;
    if (m === "profile" || m === "zcode_oauth" || m === "z_api" || m === "disabled") {
      setSelectedMethod(m);
    }
    if (state.settings.opencodeActiveModel) {
      setModel(state.settings.opencodeActiveModel);
    } else if (state.settings.defaultModel) {
      setModel(state.settings.defaultModel);
    }
    if (
      state.settings.opencodeActiveMethod === "profile" &&
      state.settings.opencodeActiveSessionId
    ) {
      setSelectedProfileId(state.settings.opencodeActiveSessionId);
    } else if (!selectedProfileId && enabledSessions[0]) {
      setSelectedProfileId(enabledSessions[0].id);
    }

    if (
      state.settings.opencodeActiveMethod === "zcode_oauth" &&
      state.settings.opencodeActiveSessionId
    ) {
      setSelectedZcodeId(state.settings.opencodeActiveSessionId);
    } else if (!selectedZcodeId && zcodeOAuthAccounts[0]) {
      setSelectedZcodeId(zcodeOAuthAccounts[0].id);
    }

    if (
      state.settings.opencodeActiveMethod === "z_api" &&
      state.settings.opencodeActiveSessionId
    ) {
      setSelectedZApiId(state.settings.opencodeActiveSessionId);
    } else if (!selectedZApiId && zApiAccounts[0]) {
      setSelectedZApiId(zApiAccounts[0].id);
    }

    if (!zcodeAccId && zcodeAccounts[0]) {
      setZcodeAccId(zcodeAccounts[0].id);
    }
  }, [
    state,
    enabledSessions,
    zcodeOAuthAccounts,
    zApiAccounts,
    zcodeAccounts,
    selectedProfileId,
    selectedZcodeId,
    selectedZApiId,
    zcodeAccId,
  ]);

  const modelOptions = Array.from(
    new Set([...MODEL_CATALOG.map((c) => c.id), ...(state?.settings.recentModels ?? []), model]),
  ).filter(Boolean);

  const hasLive = !!live?.hasAuthFile || !!live?.hasZcodeProvider;
  const oauthRunning = state?.proxy.oauthRunning ?? false;

  const kindLabel = (k: string) => {
    if (k === "opencode_zcode_proxy") return "ZCode Proxy";
    if (k === "opencode_api") return "API Key";
    return "Snapshot / OAuth";
  };

  const openNewApi = () => {
    setEditingSession(null);
    setApiName("");
    setProviderPreset("anthropic");
    setCustomProviderId("anthropic");
    setApiKey("");
    setBaseUrl("");
    setApiModel("");
    setApiOpen(true);
  };

  const openEditSession = (s: HarnessSessionSummary) => {
    if (s.kind === "opencode_api") {
      setEditingSession(s);
      setApiName(s.name);
      const pid = s.providerHint ?? "custom";
      const isPreset = PRESET_PROVIDERS.some((p) => p.id === pid && p.id !== "custom");
      setProviderPreset(isPreset ? pid : "custom");
      setCustomProviderId(pid);
      setApiKey("");
      setBaseUrl(typeof s.meta.baseUrl === "string" ? s.meta.baseUrl : "");
      setApiModel(typeof s.meta.model === "string" ? s.meta.model : "");
      setApiOpen(true);
    } else if (s.kind === "opencode_zcode_proxy") {
      setEditingZcodeBridge(s);
      setZcodeName(s.name);
      setZcodeAccId(
        typeof s.meta.zcodeAccountId === "string"
          ? s.meta.zcodeAccountId
          : zcodeAccounts[0]?.id ?? "",
      );
      setZcodeModel(typeof s.meta.model === "string" ? s.meta.model : "glm-5.3");
      setZcodeOpen(true);
    }
  };

  const applyMethod = async () => {
    if (selectedMethod === "profile") {
      await setMethod({ method: "profile", targetId: selectedProfileId });
    } else if (selectedMethod === "zcode_oauth") {
      await setMethod({ method: "zcode_oauth", targetId: selectedZcodeId, model });
    } else if (selectedMethod === "z_api") {
      await setMethod({ method: "z_api", targetId: selectedZApiId, model });
    } else {
      await setMethod({ method: "disabled" });
    }
  };

  const canApply =
    selectedMethod === "disabled" ||
    (selectedMethod === "profile" && !!selectedProfileId) ||
    (selectedMethod === "zcode_oauth" && !!selectedZcodeId) ||
    (selectedMethod === "z_api" && !!selectedZApiId);

  const methodCards: Array<{
    id: OpenCodeMethod;
    icon: React.ReactNode;
    title: string;
    desc: string;
  }> = [
    {
      id: "profile",
      icon: <ShieldCheck className="h-4 w-4 text-primary" />,
      title: t("methodOpencodeProfile"),
      desc: t("methodOpencodeProfileDesc"),
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
      id: "disabled",
      icon: <PowerOff className="h-4 w-4 text-destructive" />,
      title: t("methodDisabled"),
      desc: t("methodDisabledDesc"),
    },
  ];

  const liveBadgeText =
    live?.detectedMode === "zcode_oauth"
      ? t("methodZcodeOAuth")
      : live?.detectedMode === "z_api"
        ? t("methodZApi")
        : live?.detectedMode === "profile"
          ? t("methodOpencodeProfile")
          : t("disabledLabel");

  return (
    <>
      <PageHeader
        title={t("opencodeTitle")}
        subtitle={t("opencodeSubtitle")}
        actions={
          <>
            <DisableHarnessButton
              disabled={!hasLive}
              busy={!!busyMap["ocDisable"]}
              onClick={disable}
            />
            <Button
              variant="outline"
              size="sm"
              disabled={!live?.hasAuthFile || !!busyMap["ocCapture"]}
              onClick={() => setCaptureOpen(true)}
            >
              <Download /> {t("captureCurrentBtn")}
            </Button>
            <Button variant="outline" size="sm" onClick={openNewApi}>
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
              variant="outline"
              size="sm"
              disabled={zcodeAccounts.length === 0}
              onClick={() => {
                setEditingZcodeBridge(null);
                setZcodeName("");
                setZcodeModel("glm-5.3");
                setZcodeOpen(true);
              }}
            >
              <Zap /> ZCode Köprü Profili
            </Button>
            <Button
              size="sm"
              disabled={state?.harnessOauthRunning || !live?.cli.found}
              onClick={() => setOauthOpen(true)}
            >
              <LogIn /> OpenCode OAuth
            </Button>
          </>
        }
      />

      <HarnessOAuthBanner target="opencode" />

      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center justify-between gap-2 text-base">
            <span className="flex items-center gap-2">
              <Code2 className="h-4 w-4 text-muted-foreground" />
              Canlı OpenCode Durumu
            </span>
            <div className="flex items-center gap-2">
              <Badge variant={live?.cli.found ? "outline" : "warning"}>
                {live?.cli.found ? live.cli.version ?? t("cliInstalled") : t("cliNotInstalled")}
              </Badge>
              <Badge variant={hasLive ? "success" : "secondary"}>{liveBadgeText}</Badge>
            </div>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-x-8 md:grid-cols-2">
            <div>
              <KeyValue
                label="auth.json"
                value={live?.authProviders.length ? live.authProviders.join(", ") : t("none")}
                mono
              />
              <KeyValue
                label="opencode.json"
                value={live?.configProviders.length ? live.configProviders.join(", ") : t("none")}
                mono
              />
              <KeyValue label="Auth Yolu" value={live?.authPath ?? "—"} mono />
            </div>
            <div>
              <KeyValue label="Endpoint" value={live?.liveBaseUrl ?? "Varsayılan"} mono />
              <KeyValue label={t("model")} value={live?.liveModel ?? "Varsayılan"} mono />
              <KeyValue label="Config Yolu" value={live?.configPath ?? "—"} mono />
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
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
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

          {selectedMethod === "profile" && (
            <div className="space-y-1.5">
              <Label>Kayıtlı OpenCode Oturumu / Sağlayıcı Profili</Label>
              {enabledSessions.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  Etkin OpenCode profili yok. Yukarıdan Sağlayıcı API, OpenCode OAuth veya mevcut
                  oturumu içe aktarın.
                </p>
              ) : (
                <Select
                  value={selectedProfileId}
                  onChange={(e) => setSelectedProfileId(e.target.value)}
                >
                  {enabledSessions.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.providerHint ?? kindLabel(s.kind)})
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

          <div className="flex items-center justify-between gap-4 pt-1">
            <p className="text-xs text-muted-foreground">
              Seçilen yöntem `auth.json` ve `opencode.json` dosyalarına yazılır; terminalden `opencode`
              komutuyla doğrudan çalışır.
            </p>
            <Button
              disabled={!canApply || !!busyMap["ocMethod"]}
              variant={selectedMethod === "disabled" ? "destructive" : "default"}
              onClick={() => void applyMethod()}
            >
              {selectedMethod === "disabled" ? t("disableBtn") : t("activate")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="mb-4">
        <h2 className="mb-3 text-base font-semibold">Kayıtlı OpenCode Profilleri</h2>
        {sessions.length === 0 ? (
          <EmptyState
            icon={<Code2 className="h-10 w-10" />}
            title="Henüz OpenCode oturumu yok"
            description="Mevcut auth.json dosyanızı içe aktarın, istediğiniz sağlayıcıyla API Key / ZCode profili ekleyin veya `opencode auth login` ile oturum kaydedin."
          />
        ) : (
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
            {sessions.map((s) => (
              <SessionCard
                key={s.id}
                session={s}
                kindLabel={kindLabel(s.kind)}
                extraRows={[
                  ...(typeof s.meta.model === "string" && s.meta.model
                    ? [{ label: t("model"), value: s.meta.model, mono: true }]
                    : []),
                  ...(typeof s.meta.baseUrl === "string" && s.meta.baseUrl
                    ? [{ label: "Base URL", value: s.meta.baseUrl, mono: true }]
                    : []),
                ]}
                activateBusy={!!busyMap[`ocAct:${s.id}`]}
                onEdit={
                  s.kind === "opencode_api" || s.kind === "opencode_zcode_proxy"
                    ? () => openEditSession(s)
                    : undefined
                }
                onActivate={() => activateSession(s.id)}
              />
            ))}
          </div>
        )}
      </div>

      <FolderLauncherCard
        title="OpenCode Başlatıcı"
        disabled={!live?.cli.found}
        busy={!!busyMap["ocLaunch"]}
        onLaunch={(folder) => launchFolder(folder)}
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
              placeholder="Örn. Kişisel OpenCode"
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
                await captureCurrent(captureName.trim() || undefined);
                setCaptureName("");
                setCaptureOpen(false);
              }}
            >
              {t("save")}
            </Button>
          </div>
        </div>
      </Dialog>

      {/* Add / Edit API Dialog */}
      <Dialog
        open={apiOpen}
        onClose={() => setApiOpen(false)}
        title={editingSession ? "OpenCode API Profilini Düzenle" : "OpenCode Sağlayıcı API Ekle"}
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
              value={providerPreset}
              onChange={(e) => {
                const pid = e.target.value;
                setProviderPreset(pid);
                if (pid !== "custom") {
                  setCustomProviderId(pid);
                  const preset = PRESET_PROVIDERS.find((p) => p.id === pid);
                  if (preset) setBaseUrl(preset.baseUrl);
                }
              }}
            >
              {PRESET_PROVIDERS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label} {p.id !== "custom" ? `(${p.id})` : ""}
                </option>
              ))}
            </Select>
          </div>
          {providerPreset === "custom" && (
            <div className="space-y-1.5">
              <Label>Özel Sağlayıcı Kimliği</Label>
              <Input
                value={customProviderId}
                onChange={(e) => setCustomProviderId(e.target.value)}
                placeholder="örn. deepseek, groq, fireworks"
                className="font-mono"
              />
            </div>
          )}
          <div className="space-y-1.5">
            <Label>
              {t("apiKeyLabel")} {editingSession ? "(boş bırakılırsa mevcut korunur)" : ""}
            </Label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={editingSession ? "Değiştirmek için yeni anahtar girin" : "sk-..."}
              className="font-mono"
            />
          </div>
          <div className="space-y-1.5">
            <Label>{t("baseUrlOptional")}</Label>
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com/v1"
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
            <Button variant="outline" onClick={() => setApiOpen(false)}>
              {t("cancel")}
            </Button>
            <Button
              disabled={
                !customProviderId.trim() || (!editingSession && apiKey.trim().length < 4)
              }
              onClick={async () => {
                if (editingSession) {
                  await editSession({
                    id: editingSession.id,
                    name: apiName.trim() || undefined,
                    providerId: customProviderId.trim(),
                    apiKey: apiKey.trim() || undefined,
                    baseUrl: baseUrl.trim(),
                    model: apiModel.trim(),
                  });
                } else {
                  await addApiSession({
                    name: apiName,
                    providerId: customProviderId.trim(),
                    apiKey,
                    baseUrl: baseUrl.trim() || undefined,
                    model: apiModel.trim() || undefined,
                  });
                }
                setApiOpen(false);
                setApiName("");
                setApiKey("");
              }}
            >
              {t("save")}
            </Button>
          </div>
        </div>
      </Dialog>

      {/* Add / Edit ZCode Proxy Bridge Dialog */}
      <Dialog
        open={zcodeOpen}
        onClose={() => setZcodeOpen(false)}
        title={
          editingZcodeBridge ? "OpenCode ZCode Profilini Düzenle" : "OpenCode ZCode Köprüsü"
        }
        description="Seçilen ZCode hesabını OpenCode içinde `zcode` provider olarak tanımlar ve gerektiğinde proxy'yi otomatik başlatır."
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. OpenCode ZCode GLM-5.3"
              value={zcodeName}
              onChange={(e) => setZcodeName(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label>ZCode Hesabı</Label>
            <Select value={zcodeAccId} onChange={(e) => setZcodeAccId(e.target.value)}>
              {zcodeAccounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name} ({a.provider.toUpperCase()})
                </option>
              ))}
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label>{t("selectModel")}</Label>
            <Select
              value={zcodeModel}
              onChange={(e) => setZcodeModel(e.target.value)}
              className="font-mono"
            >
              {MODEL_CATALOG.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.id}
                </option>
              ))}
            </Select>
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setZcodeOpen(false)}>
              {t("cancel")}
            </Button>
            <Button
              disabled={!zcodeAccId}
              onClick={async () => {
                if (editingZcodeBridge) {
                  await editSession({
                    id: editingZcodeBridge.id,
                    name: zcodeName.trim() || undefined,
                    zcodeAccountId: zcodeAccId,
                    model: zcodeModel,
                  });
                } else {
                  await addZcodeSession({
                    name: zcodeName,
                    zcodeAccountId: zcodeAccId,
                    model: zcodeModel,
                  });
                }
                setZcodeOpen(false);
                setZcodeName("");
              }}
            >
              {t("save")}
            </Button>
          </div>
        </div>
      </Dialog>

      {/* OAuth Login Dialog */}
      <Dialog
        open={oauthOpen}
        onClose={() => setOauthOpen(false)}
        title="OpenCode OAuth Giriş"
        description="İzole bir `OPENCODE_AUTH_PATH` ile terminalde `opencode auth login` başlatılır."
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. OpenCode OAuth"
              value={oauthName}
              onChange={(e) => setOauthName(e.target.value)}
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setOauthOpen(false)}>
              {t("cancel")}
            </Button>
            <Button
              onClick={async () => {
                setOauthOpen(false);
                await startOAuth(oauthName.trim() || undefined);
                setOauthName("");
              }}
            >
              <LogIn /> {t("launch")}
            </Button>
          </div>
        </div>
      </Dialog>

      <ZCodeOAuthDialog open={zcodeOAuthOpen} onClose={() => setZcodeOAuthOpen(false)} />
      <ZCodeApiKeyDialog open={zcodeApiOpen} onClose={() => setZcodeApiOpen(false)} />
    </>
  );
}
