import * as React from "react";
import { CheckCircle2, KeyRound, LogIn, Pencil, Play, Trash2, UserRound } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Badge,
  Button,
  Card,
  CardContent,
  Dialog,
  Input,
  Label,
  Select,
} from "@/components/ui";
import { ConfirmDialog, CopyField, EmptyState, KeyValue, PageHeader } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import type { AccountSummary } from "@/lib/types";
import { timeAgo } from "@/lib/utils";

function OAuthDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const [provider, setProvider] = React.useState("zai");
  const [name, setName] = React.useState("");
  const startOAuth = useApp((s) => s.startOAuth);
  const cancelOAuth = useApp((s) => s.cancelOAuth);
  const oauthUrl = useApp((s) => s.oauthUrl);
  const oauthRunning = useApp((s) => s.state?.proxy.oauthRunning ?? false);
  const busy = useApp((s) => !!s.busy["oauth"]);

  React.useEffect(() => {
    if (!open) {
      setName("");
      setProvider("zai");
    }
  }, [open]);

  return (
    <Dialog
      open={open}
      onClose={() => {
        if (oauthRunning) void cancelOAuth();
        onClose();
      }}
      title={t("oauthFlowTitle")}
      description={t("oauthFlowStarted")}
      wide={!!oauthUrl || oauthRunning}
    >
      {oauthRunning || oauthUrl ? (
        <div className="space-y-4">
          {oauthUrl && (
            <>
              <div>
                <Label className="mb-1.5 block">{t("oauthUrlLabel")}</Label>
                <CopyField value={oauthUrl} />
              </div>
              <Button variant="outline" size="sm" onClick={() => void openUrl(oauthUrl)}>
                {t("oauthOpenBrowser")}
              </Button>
            </>
          )}
          <div className="flex items-center justify-between rounded-md border bg-muted/40 px-3 py-2.5">
            <span className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="h-2 w-2 animate-pulse rounded-full bg-primary" />
              {t("oauthWaiting")}
            </span>
            <Button variant="ghost" size="sm" onClick={() => void cancelOAuth()}>
              {t("oauthCancel")}
            </Button>
          </div>
        </div>
      ) : (
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="oauth-provider">{t("provider")}</Label>
            <Select
              id="oauth-provider"
              value={provider}
              onChange={(e) => setProvider(e.target.value)}
              disabled={busy}
            >
              <option value="zai">Z.AI (z.ai)</option>
              <option value="bigmodel">BigModel (bigmodel.cn)</option>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="oauth-name">{t("apiKeyName")}</Label>
            <Input
              id="oauth-name"
              placeholder="Örn. Kişisel Hesabım"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={busy}
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={onClose} disabled={busy}>
              {t("cancel")}
            </Button>
            <Button disabled={busy} onClick={() => void startOAuth(provider, name || undefined)}>
              <LogIn /> {t("addOAuth")}
            </Button>
          </div>
        </div>
      )}
    </Dialog>
  );
}

function ApiKeyDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const [name, setName] = React.useState("");
  const [provider, setProvider] = React.useState("zai");
  const [apiKey, setApiKey] = React.useState("");
  const [showKey, setShowKey] = React.useState(false);
  const addApiKey = useApp((s) => s.addApiKey);
  const busy = useApp((s) => !!s.busy["addKey"]);

  const submit = async () => {
    await addApiKey({ name, provider, apiKey });
    onClose();
    setName("");
    setApiKey("");
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={t("addApiKey")}
      description={t("apiKeyPlanNote")}
    >
      <div className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="key-name">{t("apiKeyName")}</Label>
          <Input
            id="key-name"
            placeholder="Örn. API Key Hesabım"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="key-provider">{t("apiKeyProvider")}</Label>
          <Select
            id="key-provider"
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
          >
            <option value="zai">Z.AI (api.z.ai)</option>
            <option value="bigmodel">BigModel (open.bigmodel.cn)</option>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="key-value">{t("apiKeyLabel")}</Label>
          <div className="flex gap-2">
            <Input
              id="key-value"
              type={showKey ? "text" : "password"}
              autoComplete="off"
              placeholder="••••••••••••••••"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              className="font-mono"
            />
            <Button variant="outline" size="sm" onClick={() => setShowKey((v) => !v)}>
              {showKey ? "Gizle" : "Göster"}
            </Button>
          </div>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onClose} disabled={busy}>
            {t("cancel")}
          </Button>
          <Button onClick={() => void submit()} disabled={busy || apiKey.trim().length < 8}>
            {t("save")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function EditDialog({
  account,
  onClose,
}: {
  account: AccountSummary | null;
  onClose: () => void;
}) {
  const t = useT();
  const [name, setName] = React.useState("");
  const [plan, setPlan] = React.useState("coding-plan");
  const [model, setModel] = React.useState("");
  const updateAccount = useApp((s) => s.updateAccount);
  const busy = useApp((s) => !!s.busy["updAcc"]);

  React.useEffect(() => {
    if (account) {
      setName(account.name);
      setPlan(account.defaultPlan ?? "coding-plan");
      setModel(account.defaultModel ?? "");
    }
  }, [account]);

  return (
    <Dialog open={!!account} onClose={onClose} title={t("rename")}>
      <div className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="edit-name">{t("apiKeyName")}</Label>
          <Input id="edit-name" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="edit-plan">{t("selectPlan")}</Label>
          <Select
            id="edit-plan"
            value={plan}
            onChange={(e) => setPlan(e.target.value)}
            disabled={account?.kind === "api-key"}
          >
            <option value="coding-plan">{t("planCoding")}</option>
            <option value="start-plan" disabled={account ? !account.hasJwt : false}>
              {t("planStart")}
            </option>
          </Select>
          {account?.kind === "api-key" && (
            <p className="text-xs text-muted-foreground">{t("planApiKeyLimit")}</p>
          )}
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="edit-model">{t("defaultModelLabel")}</Label>
          <Input
            id="edit-model"
            placeholder="glm-5.3"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="font-mono"
          />
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onClose} disabled={busy}>
            {t("cancel")}
          </Button>
          <Button
            disabled={busy || !account}
            onClick={async () => {
              if (!account) return;
              await updateAccount({
                id: account.id,
                name,
                defaultPlan: account.kind === "api-key" ? undefined : plan,
                defaultModel: model,
              });
              onClose();
            }}
          >
            {t("save")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function AccountCardView({ account }: { account: AccountSummary }) {
  const t = useT();
  const activate = useApp((s) => s.activateAccount);
  const remove = useApp((s) => s.deleteAccount);
  const test = useApp((s) => s.testAccount);
  const busyMap = useApp((s) => s.busy);
  const [confirmActivate, setConfirmActivate] = React.useState(false);
  const [confirmDelete, setConfirmDelete] = React.useState(false);
  const [editing, setEditing] = React.useState(false);
  const [testResult, setTestResult] = React.useState<string | null>(null);

  const busy =
    !!busyMap[`actAcc:${account.id}`] || !!busyMap[`delAcc:${account.id}`] || !!busyMap["proxy"];

  return (
    <Card className={account.active ? "border-primary/60" : undefined}>
      <CardContent className="p-4">
        <div className="mb-2 flex items-start justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            {account.kind === "oauth" ? (
              <UserRound className="h-4 w-4 shrink-0 text-primary" />
            ) : (
              <KeyRound className="h-4 w-4 shrink-0 text-warning" />
            )}
            <span className="truncate font-medium">{account.name}</span>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            <Badge variant={account.kind === "oauth" ? "default" : "warning"}>
              {account.kind === "oauth" ? "OAuth" : "API Key"}
            </Badge>
            {account.active && <Badge variant="success">{t("active")}</Badge>}
          </div>
        </div>

        <div className="text-sm">
          <KeyValue
            label={t("connectionType")}
            value={
              account.kind === "oauth"
                ? account.hasJwt
                  ? `${t("connectionOAuthCoding")} · ${t("connectionOAuthStart")}`
                  : t("connectionOAuthCoding")
                : t("connectionApiKeyStandard")
            }
          />
          <KeyValue label={t("provider")} value={account.provider === "zai" ? "Z.AI" : "BigModel"} />
          <KeyValue label={t("apiKeyLabel")} value={account.keyHint} mono />
          {account.userHint && <KeyValue label="User" value={account.userHint} mono />}
          <KeyValue
            label={t("selectPlan")}
            value={
              account.defaultPlan === "start-plan" ? t("planStart") : t("planCoding")
            }
          />
          {account.defaultModel && (
            <KeyValue label={t("defaultModelLabel")} value={account.defaultModel} mono />
          )}
          <KeyValue label={t("lastUsed")} value={timeAgo(account.lastUsedAt)} />
        </div>

        {testResult && (
          <div
            className={`mt-3 rounded-md border px-3 py-2 text-xs ${
              testResult.startsWith("✓")
                ? "border-success/40 bg-success/10 text-success-foreground"
                : "border-destructive/40 bg-destructive/10 text-destructive-foreground"
            }`}
          >
            {testResult}
          </div>
        )}

        <div className="mt-3 flex flex-wrap items-center gap-2">
          {!account.active && (
            <>
              <Button size="sm" disabled={busy} onClick={() => setConfirmActivate(true)}>
                <Play /> {t("activate")}
              </Button>
              <ConfirmDialog
                open={confirmActivate}
                onClose={() => setConfirmActivate(false)}
                onConfirm={async () => {
                  setConfirmActivate(false);
                  await activate(account.id);
                }}
                title={t("activateConfirmTitle")}
                body={t("activateConfirmBody")}
                busy={busy}
              />
            </>
          )}
          <Button
            size="sm"
            variant="outline"
            disabled={!!busyMap[`testAcc:${account.id}`]}
            onClick={async () => {
              setTestResult(t("testRunning"));
              const r = await test(account.id);
              setTestResult(r ? `${r.ok ? "✓" : "✗"} ${r.message}` : null);
            }}
          >
            {busyMap[`testAcc:${account.id}`] ? t("testRunning") : t("testConnection")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setEditing(true)}>
            <Pencil /> {t("edit")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="text-destructive hover:text-destructive"
            disabled={!!busyMap[`delAcc:${account.id}`]}
            onClick={() => setConfirmDelete(true)}
          >
            <Trash2 /> {t("delete")}
          </Button>
        </div>

        <ConfirmDialog
          open={confirmDelete}
          onClose={() => setConfirmDelete(false)}
          onConfirm={async () => {
            setConfirmDelete(false);
            await remove(account.id);
          }}
          title={t("deleteConfirmTitle")}
          body={t("deleteConfirmBody")}
          destructive
          busy={!!busyMap[`delAcc:${account.id}`]}
        />

        {editing && <EditDialog account={account} onClose={() => setEditing(false)} />}
      </CardContent>
    </Card>
  );
}

export function AccountsPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const [oauthOpen, setOauthOpen] = React.useState(false);
  const [apiKeyOpen, setApiKeyOpen] = React.useState(false);
  const oauthRunning = state?.proxy.oauthRunning ?? false;

  return (
    <>
      <PageHeader
        title={t("accountsTitle")}
        subtitle={t("accountsSubtitle")}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => setApiKeyOpen(true)}>
              <KeyRound /> {t("addApiKey")}
            </Button>
            <Button size="sm" onClick={() => setOauthOpen(true)} disabled={oauthRunning}>
              <LogIn /> {t("addOAuth")}
            </Button>
          </>
        }
      />

      {!state || state.accounts.length === 0 ? (
        <EmptyState
          icon={<UserRound className="h-10 w-10" />}
          title={t("noAccounts")}
          description={t("noAccountsBody")}
          action={
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setApiKeyOpen(true)}>
                <KeyRound /> {t("addApiKey")}
              </Button>
              <Button onClick={() => setOauthOpen(true)}>
                <LogIn /> {t("addOAuth")}
              </Button>
            </div>
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
          {state.accounts.map((a) => (
            <AccountCardView key={a.id} account={a} />
          ))}
        </div>
      )}

      <OAuthDialog open={oauthOpen} onClose={() => setOauthOpen(false)} />
      <ApiKeyDialog open={apiKeyOpen} onClose={() => setApiKeyOpen(false)} />
    </>
  );
}
