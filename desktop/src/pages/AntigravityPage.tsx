import * as React from "react";
import { Download, LogIn, Sparkles } from "lucide-react";
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
} from "@/components/ui";
import { EmptyState, KeyValue, PageHeader } from "@/components/common";
import {
  DisableHarnessButton,
  FolderLauncherCard,
  HarnessOAuthBanner,
  SessionCard,
} from "@/components/HarnessShared";
import { useApp, useT } from "@/stores/app";
import { formatDateTime } from "@/lib/utils";

export function AntigravityPage() {
  const t = useT();
  const state = useApp((s) => s.state);
  const busyMap = useApp((s) => s.busy);

  const captureCurrent = useApp((s) => s.antigravityCaptureCurrent);
  const startOAuth = useApp((s) => s.antigravityStartOAuthLogin);
  const activateSession = useApp((s) => s.antigravityActivateSession);
  const disable = useApp((s) => s.antigravityDisable);
  const launchFolder = useApp((s) => s.antigravityLaunchFolder);

  const live = state?.antigravityLive;
  const sessions = (state?.sessions ?? []).filter((s) => s.harness === "antigravity");

  const [captureOpen, setCaptureOpen] = React.useState(false);
  const [captureName, setCaptureName] = React.useState("");
  const [addOpen, setAddOpen] = React.useState(false);
  const [newName, setNewName] = React.useState("");

  const hasLive = !!live?.hasKeyringToken || !!live?.hasFileToken;

  return (
    <>
      <PageHeader
        title={t("antigravityTitle")}
        subtitle={t("antigravitySubtitle")}
        actions={
          <>
            <DisableHarnessButton
              disabled={!hasLive}
              busy={!!busyMap["agyDisable"]}
              onClick={disable}
            />
            <Button
              variant="outline"
              size="sm"
              disabled={!hasLive || !!busyMap["agyCapture"]}
              onClick={() => setCaptureOpen(true)}
            >
              <Download /> {t("captureCurrentBtn")}
            </Button>
            <Button
              size="sm"
              disabled={state?.harnessOauthRunning || !live?.cli.found}
              onClick={() => setAddOpen(true)}
            >
              <LogIn /> Google Hesabı Ekle
            </Button>
          </>
        }
      />

      <HarnessOAuthBanner target="antigravity" />

      <Card className="mb-4">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center justify-between gap-2 text-base">
            <span className="flex items-center gap-2">
              <Sparkles className="h-4 w-4 text-muted-foreground" />
              Canlı Antigravity CLI Durumu
            </span>
            <div className="flex items-center gap-2">
              <Badge variant={live?.cli.found ? "outline" : "warning"}>
                {live?.cli.found ? live.cli.version ?? t("cliInstalled") : t("cliNotInstalled")}
              </Badge>
              <Badge
                variant={!hasLive ? "secondary" : live?.isExpired ? "warning" : "success"}
              >
                {!hasLive
                  ? t("disabledLabel")
                  : live?.isExpired
                    ? "Token Süresi Dolmuş"
                    : t("active")}
              </Badge>
            </div>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-x-8 md:grid-cols-2">
            <div>
              <KeyValue
                label="Windows Keyring (gemini:antigravity)"
                value={live?.hasKeyringToken ? "Mevcut ✓" : "Boş"}
              />
              <KeyValue
                label="Dosya Aynası"
                value={live?.hasFileToken ? "Mevcut ✓" : "Boş"}
              />
            </div>
            <div>
              <KeyValue label="Token" value={live?.tokenHint ?? t("none")} mono />
              <KeyValue
                label={t("quotaExpires")}
                value={live?.expiryDate ? formatDateTime(live.expiryDate) : "—"}
              />
            </div>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">{t("antigravityRestartHint")}</p>
        </CardContent>
      </Card>

      <div className="mb-4">
        {sessions.length === 0 ? (
          <EmptyState
            icon={<Sparkles className="h-10 w-10" />}
            title="Henüz Antigravity CLI hesabı yok"
            description="Mevcut `agy` oturumunuzu içe aktarın veya yeni bir Google hesabıyla giriş yapın."
          />
        ) : (
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
            {sessions.map((s) => (
              <SessionCard
                key={s.id}
                session={s}
                kindLabel="Google OAuth"
                extraRows={
                  s.expiresAt
                    ? [{ label: t("quotaExpires"), value: formatDateTime(s.expiresAt) }]
                    : undefined
                }
                activateBusy={!!busyMap[`agyAct:${s.id}`]}
                onActivate={() => activateSession(s.id)}
              />
            ))}
          </div>
        )}
      </div>

      <FolderLauncherCard
        title="Antigravity CLI Başlatıcı"
        disabled={!live?.cli.found}
        busy={!!busyMap["agyLaunch"]}
        onLaunch={(folder) => launchFolder(folder)}
      />

      <Dialog
        open={captureOpen}
        onClose={() => setCaptureOpen(false)}
        title={t("captureCurrentBtn")}
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. Kişisel Google"
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

      <Dialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title="Yeni Antigravity CLI Hesabı"
        description="Mevcut aktif oturum korunarak terminalde `agy` Google OAuth girişi açılır."
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label>{t("apiKeyName")}</Label>
            <Input
              placeholder="Örn. İş Google Hesabı"
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
                await startOAuth(newName.trim() || undefined);
                setNewName("");
              }}
            >
              <LogIn /> {t("launch")}
            </Button>
          </div>
        </div>
      </Dialog>
    </>
  );
}
