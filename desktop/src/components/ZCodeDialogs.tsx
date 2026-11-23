import * as React from "react";
import { LogIn } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Dialog, Input, Label, Select } from "@/components/ui";
import { CopyField } from "@/components/common";
import { useApp, useT } from "@/stores/app";

export function ZCodeOAuthDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
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
            <Label htmlFor="zcode-oauth-provider">{t("provider")}</Label>
            <Select
              id="zcode-oauth-provider"
              value={provider}
              onChange={(e) => setProvider(e.target.value)}
              disabled={busy}
            >
              <option value="zai">Z.AI (z.ai)</option>
              <option value="bigmodel">BigModel (bigmodel.cn)</option>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="zcode-oauth-name">{t("apiKeyName")}</Label>
            <Input
              id="zcode-oauth-name"
              placeholder="Örn. Kişisel ZCode Hesabım"
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

export function ZCodeApiKeyDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const [name, setName] = React.useState("");
  const [provider, setProvider] = React.useState("zai");
  const [apiKey, setApiKey] = React.useState("");
  const [baseUrl, setBaseUrl] = React.useState("");
  const [showKey, setShowKey] = React.useState(false);
  const addApiKey = useApp((s) => s.addApiKey);
  const busy = useApp((s) => !!s.busy["addKey"]);

  const submit = async () => {
    await addApiKey({
      name,
      provider,
      apiKey,
      baseUrl: baseUrl.trim() || undefined,
    });
    onClose();
    setName("");
    setApiKey("");
    setBaseUrl("");
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
          <Label htmlFor="zcode-key-name">{t("apiKeyName")}</Label>
          <Input
            id="zcode-key-name"
            placeholder="Örn. Z.AI API Key"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="zcode-key-provider">{t("apiKeyProvider")}</Label>
          <Select
            id="zcode-key-provider"
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
          >
            <option value="zai">Z.AI (api.z.ai)</option>
            <option value="bigmodel">BigModel (open.bigmodel.cn)</option>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="zcode-key-value">{t("apiKeyLabel")}</Label>
          <div className="flex gap-2">
            <Input
              id="zcode-key-value"
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
        <div className="space-y-1.5">
          <Label htmlFor="zcode-key-base-url">{t("baseUrlOptional")}</Label>
          <Input
            id="zcode-key-base-url"
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
          <Button onClick={() => void submit()} disabled={busy || apiKey.trim().length < 8}>
            {t("save")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
