import * as React from "react";
import {
  CheckCircle2,
  Download,
  ExternalLink,
  Eye,
  EyeOff,
  FlaskConical,
  Pencil,
  Plus,
  Search,
  Trash2,
  XCircle,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "@/lib/api";
import {
  Badge,
  Button,
  Card,
  CardContent,
  Dialog,
  Input,
  Label,
  Select,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { ConfirmDialog, CopyField, EmptyState, PageHeader } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import type {
  AccountKind,
  CustomModelDef,
  PlanId,
  ProviderId,
  TestResult,
  UnifiedAccountSummary,
} from "@/lib/types";

const KIND_BADGE: Record<AccountKind, string> = {
  zcode_oauth: "ZCode OAuth",
  z_api: "Z.AI API Key",
  claude_oauth: "Claude OAuth",
  antigravity_oauth: "Antigravity OAuth",
  custom_provider: "Özel Model",
};

const HARNESS_LABELS: Record<string, string> = {
  claude: "Claude",
  opencode: "OpenCode",
  zcode: "ZCode",
  antigravity: "Antigravity",
  service: "Servis",
};

export function AccountsPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const oauthUrl = useApp((s) => s.oauthUrl);
  const busy = useApp((s) => s.busy);

  const addZApiAccount = useApp((s) => s.addZApiAccount);
  const addCustomProvider = useApp((s) => s.addCustomProvider);
  const updateUnifiedAccount = useApp((s) => s.updateUnifiedAccount);
  const setAccountEnabled = useApp((s) => s.setAccountEnabled);
  const deleteAccount = useApp((s) => s.deleteAccount);
  const testAccount = useApp((s) => s.testAccount);
  const importSystemAccounts = useApp((s) => s.importSystemAccounts);
  const startOAuth = useApp((s) => s.startOAuth);
  const cancelOAuth = useApp((s) => s.cancelOAuth);
  const startClaudeOAuth = useApp((s) => s.startClaudeOAuth);
  const captureClaudeOAuth = useApp((s) => s.captureClaudeOAuth);
  const startAgyOAuth = useApp((s) => s.startAgyOAuth);
  const captureAgyOAuth = useApp((s) => s.captureAgyOAuth);
  const cancelHarnessOAuth = useApp((s) => s.cancelHarnessOAuth);

  const [filter, setFilter] = React.useState<"all" | AccountKind>("all");
  const [search, setSearch] = React.useState("");
  const [tests, setTests] = React.useState<Record<string, TestResult>>({});

  const pendingAddKind = useApp((s) => s.pendingAddKind);
  const clearPendingAddKind = useApp((s) => s.clearPendingAddKind);
  const pendingEditAccountId = useApp((s) => s.pendingEditAccountId);
  const clearPendingEditAccountId = useApp((s) => s.clearPendingEditAccountId);

  // Add modal
  const [addOpen, setAddOpen] = React.useState(false);
  const [addTab, setAddTab] = React.useState<AccountKind>("claude_oauth");

  React.useEffect(() => {
    if (pendingAddKind) {
      setAddTab(pendingAddKind);
      setAddOpen(true);
      clearPendingAddKind();
    }
  }, [pendingAddKind, clearPendingAddKind]);

  // ZCode OAuth form
  const [zOauthProv, setZOauthProv] = React.useState<ProviderId>("zai");
  const [zOauthName, setZOauthName] = React.useState("");

  // Z API form
  const [zApiProv, setZApiProv] = React.useState<ProviderId>("zai");
  const [zApiName, setZApiName] = React.useState("");
  const [zApiKey, setZApiKey] = React.useState("");
  const [zApiBase, setZApiBase] = React.useState("");
  const [zApiModel, setZApiModel] = React.useState("glm-5.3");

  // Claude / Agy OAuth form
  const [cliOauthName, setCliOauthName] = React.useState("");

  // Custom Provider (`Özel Model / API`) form
  const [cpName, setCpName] = React.useState("");
  const [cpPkg, setCpPkg] = React.useState("@opencode/ai/providers/openai-compatible");
  const [cpBase, setCpBase] = React.useState("");
  const [cpKey, setCpKey] = React.useState("local-colab");
  const [cpModels, setCpModels] = React.useState<CustomModelDef[]>([
    { id: "", name: "", contextLimit: 32768, outputLimit: 4096 },
  ]);

  // Edit & Delete
  const [editTarget, setEditTarget] = React.useState<UnifiedAccountSummary | null>(null);
  const [edName, setEdName] = React.useState("");
  const [edPkg, setEdPkg] = React.useState("");
  const [edBase, setEdBase] = React.useState("");
  const [edKey, setEdKey] = React.useState("");
  const [edKeyVisible, setEdKeyVisible] = React.useState(true);
  const [edPlan, setEdPlan] = React.useState<PlanId>("coding-plan");
  const [edModel, setEdModel] = React.useState("");
  const [edModels, setEdModels] = React.useState<CustomModelDef[]>([]);
  const [deleteTarget, setDeleteTarget] = React.useState<UnifiedAccountSummary | null>(null);

  const loadEditModal = React.useCallback((acc: UnifiedAccountSummary) => {
    setEditTarget(acc);
    setEdName(acc.name);
    setEdPkg(acc.package ?? "@opencode/ai/providers/openai-compatible");
    setEdBase(acc.baseUrl ?? "");
    setEdKey("");
    setEdKeyVisible(true);
    setEdPlan(acc.defaultPlan ?? "coding-plan");
    setEdModel(acc.defaultModel ?? "");
    setEdModels(
      acc.models.length > 0
        ? acc.models
        : [{ id: "", name: "", contextLimit: 32768, outputLimit: 4096 }],
    );
    if (acc.kind === "z_api" || acc.kind === "custom_provider") {
      void api
        .getAccountSecret(acc.id)
        .then((sec) => setEdKey(sec))
        .catch(() => {});
    }
  }, []);

  React.useEffect(() => {
    if (pendingEditAccountId && state?.accounts) {
      const target = state.accounts.find((a) => a.id === pendingEditAccountId);
      clearPendingEditAccountId();
      if (target) {
        loadEditModal(target);
      }
    }
  }, [pendingEditAccountId, state?.accounts, clearPendingEditAccountId, loadEditModal]);

  if (!state) {
    return <div className="py-24 text-center text-muted-foreground">{t("loading")}</div>;
  }

  const filtered = state.accounts.filter((a) => {
    if (filter !== "all" && a.kind !== filter) return false;
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      return (
        a.name.toLowerCase().includes(q) ||
        a.provider.toLowerCase().includes(q) ||
        (a.baseUrl ?? "").toLowerCase().includes(q) ||
        a.models.some((m) => m.id.toLowerCase().includes(q))
      );
    }
    return true;
  });

  const openEdit = (acc: UnifiedAccountSummary) => {
    loadEditModal(acc);
  };

  const handleSaveEdit = async () => {
    if (!editTarget) return;
    const cleanEdModels =
      editTarget.kind === "custom_provider"
        ? edModels
            .filter((m) => m.id.trim())
            .map((m) => ({
              id: m.id.trim(),
              name: m.name.trim() || m.id.trim(),
              contextLimit: m.contextLimit ? Number(m.contextLimit) : null,
              outputLimit: m.outputLimit ? Number(m.outputLimit) : null,
            }))
        : undefined;
    await updateUnifiedAccount({
      id: editTarget.id,
      name: edName.trim(),
      package: editTarget.kind === "custom_provider" ? edPkg : undefined,
      baseUrl: edBase.trim() || undefined,
      apiKey: edKey.trim() || undefined,
      defaultPlan:
        editTarget.kind === "zcode_oauth" || editTarget.kind === "z_api" ? edPlan : undefined,
      defaultModel:
        editTarget.kind === "custom_provider"
          ? cleanEdModels?.[0]?.id ?? ""
          : edModel.trim(),
      models: cleanEdModels,
    });
    setEditTarget(null);
  };

  const handleAddCustom = async () => {
    const cleanModels = cpModels
      .filter((m) => m.id.trim())
      .map((m) => ({
        id: m.id.trim(),
        name: m.name.trim() || m.id.trim(),
        contextLimit: m.contextLimit ? Number(m.contextLimit) : null,
        outputLimit: m.outputLimit ? Number(m.outputLimit) : null,
      }));
    const autoSlug =
      cpName.trim() || cleanModels[0]?.id || `custom-${Date.now().toString(36)}`;
    await addCustomProvider({
      name: cpName.trim() || autoSlug,
      providerSlug: autoSlug,
      package: cpPkg,
      baseUrl: cpBase.trim(),
      apiKey: cpKey.trim(),
      defaultModel: cleanModels[0]?.id || undefined,
      models: cleanModels,
      syncToOpencode: false,
    });
    setAddOpen(false);
    setCpName("");
    setCpBase("");
    setCpModels([{ id: "", name: "", contextLimit: 32768, outputLimit: 4096 }]);
  };

  const handleTest = async (id: string) => {
    const res = await testAccount(id);
    setTests((prev) => ({ ...prev, [id]: res }));
  };

  return (
    <>
      <PageHeader
        title={t("accountsTitle")}
        subtitle={t("accountsSubtitle")}
        actions={
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={!!busy["import"]}
              onClick={() => void importSystemAccounts()}
            >
              <Download className="h-4 w-4" />
              {t("importSystem")}
            </Button>
            <Button size="sm" onClick={() => setAddOpen(true)}>
              <Plus className="h-4 w-4" />
              {t("addAccountModel")}
            </Button>
          </div>
        }
      />

      {/* Active ZCode / Harness OAuth banners */}
      {state.proxy.oauthRunning && (
        <Card className="mb-4 border-primary/50 bg-primary/5">
          <CardContent className="flex flex-wrap items-center justify-between gap-3 py-3">
            <div className="space-y-1 min-w-0 flex-1">
              <div className="text-sm font-medium">{t("oauthFlowTitle")}</div>
              <div className="text-xs text-muted-foreground">{t("oauthWaiting")}</div>
              {oauthUrl && <CopyField value={oauthUrl} />}
            </div>
            <div className="flex gap-2">
              {oauthUrl && (
                <Button size="sm" variant="outline" onClick={() => void openUrl(oauthUrl)}>
                  <ExternalLink className="h-3.5 w-3.5" />
                  {t("oauthOpenBrowser")}
                </Button>
              )}
              <Button size="sm" variant="destructive" onClick={() => void cancelOAuth()}>
                {t("oauthCancel")}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {state.harnessOauthRunning && (
        <Card className="mb-4 border-primary/50 bg-primary/5">
          <CardContent className="flex items-center justify-between gap-3 py-3">
            <div className="text-sm">
              {t("harnessOAuthWaiting")} ({state.harnessOauthTarget})
            </div>
            <Button size="sm" variant="destructive" onClick={() => void cancelHarnessOAuth()}>
              {t("cancel")}
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Filter & Search Bar */}
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap gap-1.5">
          {(
            [
              ["all", t("all")],
              ["zcode_oauth", "ZCode OAuth"],
              ["z_api", "Z.AI API"],
              ["claude_oauth", "Claude OAuth"],
              ["antigravity_oauth", "Antigravity"],
              ["custom_provider", "Özel Model"],
            ] as const
          ).map(([id, label]) => (
            <Button
              key={id}
              size="sm"
              variant={filter === id ? "default" : "outline"}
              onClick={() => setFilter(id)}
            >
              {label}
            </Button>
          ))}
        </div>

        <div className="relative w-64">
          <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            className="pl-8"
            placeholder={t("search")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
      </div>

      {/* Vault List */}
      {filtered.length === 0 ? (
        <EmptyState
          title={t("noAccounts")}
          description={t("noAccountsBody")}
          action={
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => void importSystemAccounts()}>
                <Download className="h-4 w-4" />
                {t("importSystem")}
              </Button>
              <Button onClick={() => setAddOpen(true)}>
                <Plus className="h-4 w-4" />
                {t("addAccountModel")}
              </Button>
            </div>
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {filtered.map((acc) => {
            const tr = tests[acc.id];
            const accBusy = !!busy[`acc:${acc.id}`] || !!busy[`test:${acc.id}`];
            return (
              <Card key={acc.id} className={!acc.enabled ? "opacity-65" : ""}>
                <CardContent className="p-4 space-y-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 space-y-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-semibold text-sm truncate">{acc.name}</span>
                        <Badge variant="secondary" className="text-[11px]">
                          {KIND_BADGE[acc.kind]}
                        </Badge>
                        {acc.provider && (
                          <Badge variant="outline" className="font-mono text-[11px]">
                            {acc.provider}
                          </Badge>
                        )}
                      </div>
                      <div className="flex flex-wrap gap-1.5 text-xs text-muted-foreground">
                        {acc.keyHint && <span className="font-mono">{acc.keyHint}</span>}
                        {acc.defaultPlan && <span>• {acc.defaultPlan}</span>}
                        {acc.defaultModel && (
                          <span className="font-mono">• {acc.defaultModel}</span>
                        )}
                      </div>
                      {acc.baseUrl && (
                        <div className="truncate font-mono text-[11px] text-muted-foreground">
                          {acc.baseUrl}
                        </div>
                      )}
                    </div>

                    <Switch
                      checked={acc.enabled}
                      disabled={accBusy}
                      onCheckedChange={(v) => void setAccountEnabled(acc.id, v)}
                    />
                  </div>

                  {acc.models.length > 0 && (
                    <div className="flex flex-wrap gap-1.5">
                      {acc.models.map((m) => (
                        <Badge
                          key={m.id}
                          variant="outline"
                          className="font-mono text-[10px] font-normal"
                        >
                          {m.id}
                          {m.contextLimit ? ` (${m.contextLimit})` : ""}
                        </Badge>
                      ))}
                    </div>
                  )}

                  <div className="flex flex-wrap items-center justify-between gap-2 pt-1 border-t">
                    <div className="flex flex-wrap gap-1">
                      <span className="text-[11px] text-muted-foreground">
                        Uyumlu:{" "}
                        {acc.supportedHarnesses.map((h) => HARNESS_LABELS[h] ?? h).join(", ")}
                      </span>
                    </div>

                    <div className="flex items-center gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={accBusy}
                        onClick={() => void handleTest(acc.id)}
                        title={t("test")}
                      >
                        <FlaskConical className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={accBusy}
                        onClick={() => openEdit(acc)}
                        title={t("edit")}
                      >
                        <Pencil className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={accBusy}
                        onClick={() => setDeleteTarget(acc)}
                        title={t("delete")}
                      >
                        <Trash2 className="h-3.5 w-3.5 text-destructive" />
                      </Button>
                    </div>
                  </div>

                  {tr && (
                    <div
                      className={`flex items-center gap-1.5 text-xs ${
                        tr.ok ? "text-success" : "text-destructive"
                      }`}
                    >
                      {tr.ok ? (
                        <CheckCircle2 className="h-3.5 w-3.5 shrink-0" />
                      ) : (
                        <XCircle className="h-3.5 w-3.5 shrink-0" />
                      )}
                      <span>{tr.message}</span>
                    </div>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      {/* + Add Account / Custom Model Modal */}
      <Dialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title={t("addAccountModel")}
        wide
      >
        <Tabs value={addTab} onValueChange={(v) => setAddTab(v as AccountKind)}>
          <TabsList className="w-full flex-wrap h-auto gap-1 justify-start">
            <TabsTrigger value="claude_oauth">Claude OAuth</TabsTrigger>
            <TabsTrigger value="custom_provider">Özel Model / API</TabsTrigger>
            <TabsTrigger value="antigravity_oauth">Antigravity OAuth</TabsTrigger>
            <TabsTrigger value="z_api">Z.AI API Key</TabsTrigger>
            <TabsTrigger value="zcode_oauth">ZCode OAuth</TabsTrigger>
          </TabsList>

          {/* 1. Custom Provider / Model */}
          <TabsContent value="custom_provider" className="space-y-3 max-h-[60vh] overflow-y-auto pr-1">
            <div className="space-y-1">
              <Label>Sağlayıcı Adı</Label>
              <Input
                value={cpName}
                onChange={(e) => setCpName(e.target.value)}
                placeholder="Örn: Colab Qwen3.5"
              />
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div className="space-y-1">
                <Label>API Uyumluluğu</Label>
                <Select value={cpPkg} onChange={(e) => setCpPkg(e.target.value)}>
                  <option value="@opencode/ai/providers/openai-compatible">
                    OpenAI Uyumlu (openai-compatible)
                  </option>
                  <option value="@opencode/ai/providers/anthropic">
                    Anthropic Uyumlu (anthropic)
                  </option>
                </Select>
              </div>
              <div className="space-y-1">
                <Label>API Anahtarı (`apiKey`)</Label>
                <Input
                  value={cpKey}
                  onChange={(e) => setCpKey(e.target.value)}
                  placeholder="local-colab veya sk-..."
                />
              </div>
            </div>

            <div className="space-y-1">
              <Label>Base URL</Label>
              <Input
                className="font-mono text-xs"
                value={cpBase}
                onChange={(e) => setCpBase(e.target.value)}
                placeholder="https://....trycloudflare.com/v1"
              />
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label>Modeller</Label>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    setCpModels((prev) => [
                      ...prev,
                      { id: "", name: "", contextLimit: 32768, outputLimit: 4096 },
                    ])
                  }
                >
                  <Plus className="h-3.5 w-3.5" />
                  Model Ekle
                </Button>
              </div>
              {cpModels.map((m, idx) => (
                <div key={idx} className="grid grid-cols-12 gap-2">
                  <Input
                    className="col-span-5 font-mono text-xs"
                    placeholder="Model ID (ör: Qwen3.5.gguf)"
                    value={m.id}
                    onChange={(e) =>
                      setCpModels((prev) =>
                        prev.map((row, i) => (i === idx ? { ...row, id: e.target.value } : row)),
                      )
                    }
                  />
                  <Input
                    className="col-span-3 text-xs"
                    placeholder="Model Adı"
                    value={m.name}
                    onChange={(e) =>
                      setCpModels((prev) =>
                        prev.map((row, i) =>
                          i === idx ? { ...row, name: e.target.value } : row,
                        ),
                      )
                    }
                  />
                  <Input
                    className="col-span-2 text-xs"
                    type="number"
                    placeholder="Context"
                    value={m.contextLimit ?? ""}
                    onChange={(e) =>
                      setCpModels((prev) =>
                        prev.map((row, i) =>
                          i === idx
                            ? {
                                ...row,
                                contextLimit: e.target.value ? Number(e.target.value) : null,
                              }
                            : row,
                        ),
                      )
                    }
                  />
                  <Input
                    className="col-span-2 text-xs"
                    type="number"
                    placeholder="Output"
                    value={m.outputLimit ?? ""}
                    onChange={(e) =>
                      setCpModels((prev) =>
                        prev.map((row, i) =>
                          i === idx
                            ? {
                                ...row,
                                outputLimit: e.target.value ? Number(e.target.value) : null,
                              }
                            : row,
                        ),
                      )
                    }
                  />
                </div>
              ))}
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <Button variant="outline" onClick={() => setAddOpen(false)}>
                {t("cancel")}
              </Button>
              <Button
                disabled={!!busy["vault"] || !cpBase.trim()}
                onClick={() => void handleAddCustom()}
              >
                {t("add")}
              </Button>
            </div>
          </TabsContent>

          {/* 2. ZCode OAuth */}
          <TabsContent value="zcode_oauth" className="space-y-4">
            <div className="space-y-1.5">
              <Label>Sağlayıcı</Label>
              <Select
                value={zOauthProv}
                onChange={(e) => setZOauthProv(e.target.value as ProviderId)}
              >
                <option value="zai">Z.AI (z.ai)</option>
                <option value="bigmodel">BigModel (open.bigmodel.cn)</option>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Hesap Adı (isteğe bağlı)</Label>
              <Input
                value={zOauthName}
                onChange={(e) => setZOauthName(e.target.value)}
                placeholder="Kişisel ZCode Hesabım"
              />
            </div>
            <p className="text-xs text-muted-foreground">
              Tarayıcıda ZCode OAuth sayfası açılır. Giriş sonrası hesap Claude Code, OpenCode ve ZCode üzerinde kullanılabilir olur.
            </p>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setAddOpen(false)}>
                {t("cancel")}
              </Button>
              <Button
                disabled={!!busy["oauth"] || state.proxy.oauthRunning}
                onClick={async () => {
                  await startOAuth(zOauthProv, zOauthName.trim() || undefined);
                  setAddOpen(false);
                  setZOauthName("");
                }}
              >
                Tarayıcıyla Giriş Başlat
              </Button>
            </div>
          </TabsContent>

          {/* 3. Z.AI / BigModel API Key */}
          <TabsContent value="z_api" className="space-y-3">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div className="space-y-1">
                <Label>Sağlayıcı</Label>
                <Select
                  value={zApiProv}
                  onChange={(e) => setZApiProv(e.target.value as ProviderId)}
                >
                  <option value="zai">Z.AI</option>
                  <option value="bigmodel">BigModel</option>
                </Select>
              </div>
              <div className="space-y-1">
                <Label>Hesap Adı</Label>
                <Input
                  value={zApiName}
                  onChange={(e) => setZApiName(e.target.value)}
                  placeholder="Z.AI API Anahtarım"
                />
              </div>
            </div>
            <div className="space-y-1">
              <Label>API Key</Label>
              <Input
                type="password"
                className="font-mono text-xs"
                value={zApiKey}
                onChange={(e) => setZApiKey(e.target.value)}
                placeholder="id.secret"
              />
            </div>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div className="space-y-1">
                <Label>Varsayılan Model</Label>
                <Input
                  className="font-mono text-xs"
                  value={zApiModel}
                  onChange={(e) => setZApiModel(e.target.value)}
                  placeholder="glm-5.3"
                />
              </div>
              <div className="space-y-1">
                <Label>Özel Base URL (isteğe bağlı)</Label>
                <Input
                  className="font-mono text-xs"
                  value={zApiBase}
                  onChange={(e) => setZApiBase(e.target.value)}
                  placeholder="https://api.z.ai/api/anthropic"
                />
              </div>
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <Button variant="outline" onClick={() => setAddOpen(false)}>
                {t("cancel")}
              </Button>
              <Button
                disabled={!!busy["vault"] || zApiKey.trim().length < 8}
                onClick={async () => {
                  await addZApiAccount({
                    name: zApiName.trim(),
                    provider: zApiProv,
                    apiKey: zApiKey.trim(),
                    defaultModel: zApiModel.trim() || undefined,
                    baseUrl: zApiBase.trim() || undefined,
                  });
                  setAddOpen(false);
                  setZApiName("");
                  setZApiKey("");
                  setZApiBase("");
                }}
              >
                {t("add")}
              </Button>
            </div>
          </TabsContent>

          {/* 4. Claude OAuth */}
          <TabsContent value="claude_oauth" className="space-y-4">
            <div className="space-y-1.5">
              <Label>Oturum Adı (isteğe bağlı)</Label>
              <Input
                value={cliOauthName}
                onChange={(e) => setCliOauthName(e.target.value)}
                placeholder="Claude Pro (İş)"
              />
            </div>
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                disabled={!!busy["captureClaude"]}
                onClick={async () => {
                  await captureClaudeOAuth(cliOauthName.trim() || undefined);
                  setAddOpen(false);
                  setCliOauthName("");
                }}
              >
                Aktif `~/.claude` Oturumunu İçe Aktar
              </Button>
              <Button
                disabled={!!busy["harnessOauth"]}
                onClick={async () => {
                  await startClaudeOAuth(cliOauthName.trim() || undefined);
                  setAddOpen(false);
                  setCliOauthName("");
                }}
              >
                Yeni Claude OAuth Girişi Başlat
              </Button>
            </div>
          </TabsContent>

          {/* 5. Antigravity OAuth */}
          <TabsContent value="antigravity_oauth" className="space-y-4">
            <div className="space-y-1.5">
              <Label>Hesap Adı (isteğe bağlı)</Label>
              <Input
                value={cliOauthName}
                onChange={(e) => setCliOauthName(e.target.value)}
                placeholder="Antigravity (Kişisel)"
              />
            </div>
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                disabled={!!busy["captureAgy"]}
                onClick={async () => {
                  await captureAgyOAuth(cliOauthName.trim() || undefined);
                  setAddOpen(false);
                  setCliOauthName("");
                }}
              >
                Aktif `agy` Oturumunu İçe Aktar
              </Button>
              <Button
                disabled={!!busy["harnessOauth"]}
                onClick={async () => {
                  await startAgyOAuth(cliOauthName.trim() || undefined);
                  setAddOpen(false);
                  setCliOauthName("");
                }}
              >
                Yeni Google (`agy`) Girişi Başlat
              </Button>
            </div>
          </TabsContent>
        </Tabs>
      </Dialog>

      {/* Edit Dialog */}
      <Dialog
        open={!!editTarget}
        onClose={() => setEditTarget(null)}
        title={`${t("edit")}: ${editTarget?.name ?? ""}`}
        wide={editTarget?.kind === "custom_provider"}
        footer={
          <>
            <Button variant="outline" onClick={() => setEditTarget(null)}>
              {t("cancel")}
            </Button>
            <Button onClick={() => void handleSaveEdit()}>{t("save")}</Button>
          </>
        }
      >
        {editTarget && (
          <div className="space-y-3 max-h-[60vh] overflow-y-auto pr-1">
            <div className="space-y-1">
              <Label>Hesap / Sağlayıcı Adı</Label>
              <Input value={edName} onChange={(e) => setEdName(e.target.value)} />
            </div>

            {(editTarget.kind === "z_api" || editTarget.kind === "custom_provider") && (
              <>
                <div className="space-y-1">
                  <Label>Base URL</Label>
                  <Input
                    className="font-mono text-xs"
                    value={edBase}
                    onChange={(e) => setEdBase(e.target.value)}
                  />
                </div>
                <div className="space-y-1">
                  <Label>API Anahtarı</Label>
                  <div className="flex gap-1.5">
                    <Input
                      type={edKeyVisible ? "text" : "password"}
                      className="font-mono text-xs"
                      value={edKey}
                      onChange={(e) => setEdKey(e.target.value)}
                      placeholder="API Key"
                    />
                    <Button
                      type="button"
                      size="icon"
                      variant="outline"
                      onClick={() => setEdKeyVisible((v) => !v)}
                      title={edKeyVisible ? "Gizle" : "Göster"}
                    >
                      {edKeyVisible ? (
                        <EyeOff className="h-4 w-4" />
                      ) : (
                        <Eye className="h-4 w-4" />
                      )}
                    </Button>
                  </div>
                </div>
              </>
            )}

            {editTarget.kind === "zcode_oauth" && (
              <div className="space-y-1">
                <Label>Varsayılan Plan</Label>
                <Select
                  value={edPlan}
                  onChange={(e) => setEdPlan(e.target.value as PlanId)}
                >
                  <option value="coding-plan">Coding Plan</option>
                  {editTarget.hasJwt && <option value="start-plan">Start Plan</option>}
                </Select>
              </div>
            )}

            {(editTarget.kind === "zcode_oauth" || editTarget.kind === "z_api") && (
              <div className="space-y-1">
                <Label>Varsayılan Model</Label>
                <Input
                  className="font-mono text-xs"
                  value={edModel}
                  onChange={(e) => setEdModel(e.target.value)}
                />
              </div>
            )}

            {editTarget.kind === "custom_provider" && (
              <>
                <div className="space-y-1">
                  <Label>API Uyumluluğu</Label>
                  <Select value={edPkg} onChange={(e) => setEdPkg(e.target.value)}>
                    <option value="@opencode/ai/providers/openai-compatible">
                      OpenAI Uyumlu (openai-compatible)
                    </option>
                    <option value="@opencode/ai/providers/anthropic">
                      Anthropic Uyumlu (anthropic)
                    </option>
                  </Select>
                </div>
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <Label>Modeller</Label>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() =>
                        setEdModels((prev) => [
                          ...prev,
                          { id: "", name: "", contextLimit: 32768, outputLimit: 4096 },
                        ])
                      }
                    >
                      <Plus className="h-3.5 w-3.5" />
                      Satır Ekle
                    </Button>
                  </div>
                  {edModels.map((m, idx) => (
                    <div key={idx} className="grid grid-cols-12 gap-2">
                      <Input
                        className="col-span-5 font-mono text-xs"
                        placeholder="Model ID"
                        value={m.id}
                        onChange={(e) =>
                          setEdModels((prev) =>
                            prev.map((row, i) => (i === idx ? { ...row, id: e.target.value } : row)),
                          )
                        }
                      />
                      <Input
                        className="col-span-3 text-xs"
                        placeholder="Adı"
                        value={m.name}
                        onChange={(e) =>
                          setEdModels((prev) =>
                            prev.map((row, i) =>
                              i === idx ? { ...row, name: e.target.value } : row,
                            ),
                          )
                        }
                      />
                      <Input
                        className="col-span-2 text-xs"
                        type="number"
                        placeholder="Context"
                        value={m.contextLimit ?? ""}
                        onChange={(e) =>
                          setEdModels((prev) =>
                            prev.map((row, i) =>
                              i === idx
                                ? {
                                    ...row,
                                    contextLimit: e.target.value ? Number(e.target.value) : null,
                                  }
                                : row,
                            ),
                          )
                        }
                      />
                      <Input
                        className="col-span-2 text-xs"
                        type="number"
                        placeholder="Output"
                        value={m.outputLimit ?? ""}
                        onChange={(e) =>
                          setEdModels((prev) =>
                            prev.map((row, i) =>
                              i === idx
                                ? {
                                    ...row,
                                    outputLimit: e.target.value ? Number(e.target.value) : null,
                                  }
                                : row,
                            ),
                          )
                        }
                      />
                    </div>
                  ))}
                </div>
              </>
            )}
          </div>
        )}
      </Dialog>

      <ConfirmDialog
        open={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onConfirm={async () => {
          if (deleteTarget) await deleteAccount(deleteTarget.id);
          setDeleteTarget(null);
        }}
        title={t("deleteConfirmTitle")}
        body={t("deleteConfirmBody")}
        destructive
        busy={deleteTarget ? !!busy[`acc:${deleteTarget.id}`] : false}
      />
    </>
  );
}
