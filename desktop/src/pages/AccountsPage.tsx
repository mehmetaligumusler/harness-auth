import * as React from "react";
import { KeyRound, LogIn, Pencil, Play, PowerOff, Trash2, UserRound } from "lucide-react";
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
} from "@/components/ui";
import { ConfirmDialog, EmptyState, KeyValue, PageHeader } from "@/components/common";
import { ZCodeApiKeyDialog, ZCodeOAuthDialog } from "@/components/ZCodeDialogs";
import { useApp, useT } from "@/stores/app";
import type { AccountSummary } from "@/lib/types";
import { timeAgo } from "@/lib/utils";

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
  const [baseUrl, setBaseUrl] = React.useState("");
  const updateAccount = useApp((s) => s.updateAccount);
  const busy = useApp((s) => !!s.busy["updAcc"]);

  React.useEffect(() => {
    if (account) {
      setName(account.name);
      setPlan(account.defaultPlan ?? "coding-plan");
      setModel(account.defaultModel ?? "");
      setBaseUrl(account.baseUrl ?? "");
    }
  }, [account]);

  return (
    <Dialog open={!!account} onClose={onClose} title={t("edit")}>
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
        <div className="space-y-1.5">
          <Label htmlFor="edit-baseurl">{t("baseUrlOptional")}</Label>
          <Input
            id="edit-baseurl"
            placeholder="https://api.z.ai/api/anthropic"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
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
                baseUrl,
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
  const deactivate = useApp((s) => s.deactivateAccount);
  const setAccountEnabled = useApp((s) => s.setAccountEnabled);
  const remove = useApp((s) => s.deleteAccount);
  const test = useApp((s) => s.testAccount);
  const busyMap = useApp((s) => s.busy);
  const [confirmActivate, setConfirmActivate] = React.useState(false);
  const [confirmDelete, setConfirmDelete] = React.useState(false);
  const [editing, setEditing] = React.useState(false);
  const [testResult, setTestResult] = React.useState<string | null>(null);

  const busy =
    !!busyMap[`actAcc:${account.id}`] ||
    !!busyMap[`enAcc:${account.id}`] ||
    !!busyMap[`delAcc:${account.id}`] ||
    !!busyMap["deactAcc"] ||
    !!busyMap["proxy"];

  return (
    <Card
      className={
        account.active
          ? "border-primary/60"
          : !account.enabled
            ? "opacity-60"
            : undefined
      }
    >
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
          <div className="flex shrink-0 items-center gap-2">
            <Badge variant={account.kind === "oauth" ? "default" : "warning"}>
              {account.kind === "oauth" ? "OAuth" : "API Key"}
            </Badge>
            {account.active && <Badge variant="success">{t("active")}</Badge>}
            <Switch
              checked={account.enabled}
              disabled={busy}
              onCheckedChange={(v) => void setAccountEnabled(account.id, v)}
            />
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
          {account.baseUrl && <KeyValue label="Base URL" value={account.baseUrl} mono />}
          <KeyValue
            label={t("selectPlan")}
            value={account.defaultPlan === "start-plan" ? t("planStart") : t("planCoding")}
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
          {account.active ? (
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => void deactivate()}
            >
              <PowerOff /> {t("deactivateBtn")}
            </Button>
          ) : (
            <>
              <Button
                size="sm"
                disabled={busy || !account.enabled}
                onClick={() => setConfirmActivate(true)}
              >
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

      <ZCodeOAuthDialog open={oauthOpen} onClose={() => setOauthOpen(false)} />
      <ZCodeApiKeyDialog open={apiKeyOpen} onClose={() => setApiKeyOpen(false)} />
    </>
  );
}
