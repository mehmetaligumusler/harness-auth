import * as React from "react";
import { Bot, Check, FlaskConical, Plus, Search, Star } from "lucide-react";
import { toast } from "sonner";
import { Badge, Button, Card, CardContent, Input, Tabs, TabsList, TabsTrigger } from "@/components/ui";
import { EmptyState, PageHeader } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import { api } from "@/lib/api";
import { MODEL_CATALOG } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import type { ModelsInfo } from "@/lib/types";

export function ModelsPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const [info, setInfo] = React.useState<ModelsInfo | null>(null);
  const [search, setSearch] = React.useState("");
  const [testing, setTesting] = React.useState<string | null>(null);
  const [testResults, setTestResults] = React.useState<Record<string, string>>({});
  const [customInput, setCustomInput] = React.useState("");
  const [tab, setTab] = React.useState("all");

  const load = React.useCallback(() => {
    api.listModels().then(setInfo).catch(() => setInfo(null));
  }, []);

  React.useEffect(() => {
    load();
  }, [load, state?.proxy.loggedIn]);

  const running = !!state?.proxy.processAlive && !!state?.proxy.serverRunning;
  const defaultModel = info?.defaultModel ?? state?.settings.defaultModel ?? "glm-5.3";
  const customModels = (info?.configModels ?? []).filter(
    (m) => !MODEL_CATALOG.some((c) => c.id === m),
  );
  const allModels = [
    ...MODEL_CATALOG.map((c) => c.id),
    ...(info?.endpointModels ?? []).filter((m) => !MODEL_CATALOG.some((c) => c.id === m)),
    ...customModels,
  ];
  const recent = state?.settings.recentModels ?? [];

  const filtered = (tab === "recent" ? recent : allModels).filter((m) =>
    m.toLowerCase().includes(search.toLowerCase()),
  );

  const setDefault = async (model: string) => {
    try {
      await api.saveModelPrefs({ defaultModel: model });
      toast.success(t("saved"));
      load();
    } catch (e) {
      toast.error(t("errGeneric"), { description: String(e) });
    }
  };

  const testModel = async (model: string) => {
    setTesting(model);
    try {
      const r = await api.testModel(model);
      setTestResults((prev) => ({ ...prev, [model]: `${r.ok ? "✓" : "✗"} ${r.message}` }));
    } catch (e) {
      setTestResults((prev) => ({ ...prev, [model]: `✗ ${String(e)}` }));
    } finally {
      setTesting(null);
    }
  };

  const addCustom = async () => {
    const name = customInput.trim();
    if (!name) return;
    try {
      const merged = Array.from(new Set([...(info?.configModels ?? []), name]));
      await api.saveModelPrefs({ customModels: merged });
      setCustomInput("");
      toast.success(t("saved"));
      load();
    } catch (e) {
      toast.error(t("errGeneric"), { description: String(e) });
    }
  };

  const ModelRow = ({ id }: { id: string }) => {
    const cat = MODEL_CATALOG.find((c) => c.id === id);
    const isDefault = id === defaultModel;
    const result = testResults[id];
    return (
      <div
        className={cn(
          "flex items-center justify-between gap-3 rounded-md border px-4 py-3",
          isDefault && "border-primary/60 bg-primary/5",
        )}
      >
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <code className="font-mono text-sm font-medium">{id}</code>
            {isDefault && <Badge variant="default">{t("modelsDefault")}</Badge>}
            {info?.endpointModels.includes(id) && (
              <Badge variant="secondary">{t("modelsAdvertised")}</Badge>
            )}
            {!cat && <Badge variant="outline">{t("modelsCustom")}</Badge>}
          </div>
          {cat && (
            <div className="mt-0.5 text-xs text-muted-foreground">
              {t("modelsContext")}: {(cat.contextWindow / 1000).toLocaleString()}K ·{" "}
              {t("modelsOutput")}: {(cat.maxOutputTokens / 1000).toLocaleString()}K
              {cat.reasoning && ` · ${t("featureReasoning")}`}
              {cat.vision && ` · ${t("featureVision")}`}
            </div>
          )}
          {result && (
            <div className={cn("mt-1 text-xs", result.startsWith("✓") ? "text-success" : "text-destructive")}>
              {result}
            </div>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={!running || testing === id}
            onClick={() => void testModel(id)}
            title={t("modelsTestNote")}
          >
            <FlaskConical /> {testing === id ? t("testRunning") : t("test")}
          </Button>
          {!isDefault && (
            <Button variant="ghost" size="sm" onClick={() => void setDefault(id)}>
              <Star /> {t("modelsSetDefault")}
            </Button>
          )}
        </div>
      </div>
    );
  };

  return (
    <>
      <PageHeader
        title={t("modelsTitle")}
        subtitle={t("modelsSubtitle")}
        actions={
          <div className="relative">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              className="w-64 pl-8"
              placeholder={t("modelsSearchPlaceholder")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
        }
      />

      {!running && (
        <div className="mb-4 rounded-md border border-warning/40 bg-warning/10 px-4 py-2.5 text-sm text-warning-foreground">
          {t("modelsProxyOff")}
        </div>
      )}

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Tabs value={tab} onValueChange={setTab}>
          <TabsList>
            <TabsTrigger value="all">{t("modelsCatalog")}</TabsTrigger>
            <TabsTrigger value="recent">{t("modelsRecent")}</TabsTrigger>
          </TabsList>
        </Tabs>
        <div className="ml-auto flex items-end gap-2">
          <div>
            <Input
              className="w-56 font-mono"
              placeholder={t("modelsCustomName")}
              value={customInput}
              onChange={(e) => setCustomInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void addCustom()}
            />
          </div>
          <Button variant="outline" size="sm" onClick={() => void addCustom()}>
            <Plus /> {t("modelsCustomAdd")}
          </Button>
        </div>
      </div>

      <p className="mb-3 text-xs text-muted-foreground">{t("modelsVerifyNote")}</p>

      {filtered.length === 0 ? (
        <EmptyState
          icon={<Bot className="h-10 w-10" />}
          title={tab === "recent" ? t("modelsRecent") : t("modelsTitle")}
          description={tab === "recent" ? t("none") : undefined}
        />
      ) : (
        <div className="space-y-2">
          {filtered.map((m) => (
            <ModelRow key={m} id={m} />
          ))}
        </div>
      )}
    </>
  );
}
