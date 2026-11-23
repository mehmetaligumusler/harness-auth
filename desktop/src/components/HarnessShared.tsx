import * as React from "react";
import { FolderOpen, Loader2, Pencil, Play, PowerOff, Terminal, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
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
  Switch,
} from "@/components/ui";
import { ConfirmDialog, KeyValue } from "@/components/common";
import { useApp, useT } from "@/stores/app";
import type { HarnessId, HarnessSessionSummary } from "@/lib/types";
import { timeAgo } from "@/lib/utils";

export function HarnessOAuthBanner({ target }: { target: HarnessId }) {
  const t = useT();
  const running = useApp((s) => s.state?.harnessOauthRunning ?? false);
  const currentTarget = useApp((s) => s.state?.harnessOauthTarget);
  const cancel = useApp((s) => s.cancelHarnessOAuth);

  if (!running || currentTarget !== target) return null;

  return (
    <div className="mb-4 flex items-center justify-between gap-3 rounded-lg border border-primary/40 bg-primary/10 px-4 py-3 text-sm">
      <span className="flex items-center gap-2 text-foreground">
        <Loader2 className="h-4 w-4 animate-spin text-primary" />
        {t("harnessOAuthWaiting")}
      </span>
      <Button variant="outline" size="sm" onClick={() => void cancel()}>
        {t("cancel")}
      </Button>
    </div>
  );
}

export function FolderLauncherCard({
  title,
  disabled,
  busy,
  onLaunch,
}: {
  title?: string;
  disabled?: boolean;
  busy?: boolean;
  onLaunch: (folder?: string) => Promise<void>;
}) {
  const t = useT();
  const state = useApp((s) => s.state);
  const [folder, setFolder] = React.useState("");

  React.useEffect(() => {
    if (!folder && state?.settings.claudeDefaultFolder) {
      setFolder(state.settings.claudeDefaultFolder);
    } else if (!folder && state?.settings.recentFolders[0]) {
      setFolder(state.settings.recentFolders[0]);
    }
  }, [state?.settings.claudeDefaultFolder, state?.settings.recentFolders, folder]);

  const pickFolder = async () => {
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") setFolder(picked);
    } catch {
      /* cancelled */
    }
  };

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Terminal className="h-4 w-4 text-muted-foreground" />
          {title ?? t("launchTerminalBtn")}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1.5">
          <Label>{t("projectFolder")}</Label>
          <div className="flex gap-2">
            <Input
              value={folder}
              onChange={(e) => setFolder(e.target.value)}
              placeholder="C:\\Projects\\my-app"
              className="font-mono"
            />
            <Button variant="outline" onClick={() => void pickFolder()}>
              <FolderOpen /> {t("pickFolder")}
            </Button>
            <Button
              disabled={disabled || busy}
              onClick={() => void onLaunch(folder.trim() || undefined)}
            >
              <Play /> {t("launch")}
            </Button>
          </div>
          {(state?.settings.recentFolders?.length ?? 0) > 0 && (
            <div className="flex flex-wrap items-center gap-1.5 pt-1">
              <span className="text-xs text-muted-foreground">{t("recentFolders")}:</span>
              {state?.settings.recentFolders.slice(0, 5).map((f) => (
                <button
                  key={f}
                  type="button"
                  onClick={() => setFolder(f)}
                  className="rounded border bg-muted/50 px-1.5 py-0.5 font-mono text-[11px] hover:bg-accent"
                  title={f}
                >
                  {f.length > 40 ? `…${f.slice(-38)}` : f}
                </button>
              ))}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

export function SessionCard({
  session,
  kindLabel,
  extraRows,
  onActivate,
  onEdit,
  activateBusy,
}: {
  session: HarnessSessionSummary;
  kindLabel: string;
  extraRows?: Array<{ label: string; value: string; mono?: boolean }>;
  onActivate: () => Promise<void>;
  onEdit?: () => void;
  activateBusy?: boolean;
}) {
  const t = useT();
  const updateSession = useApp((s) => s.updateSession);
  const setSessionEnabled = useApp((s) => s.setSessionEnabled);
  const deleteSession = useApp((s) => s.deleteSession);
  const busyMap = useApp((s) => s.busy);

  const [renaming, setRenaming] = React.useState(false);
  const [newName, setNewName] = React.useState(session.name);
  const [confirmDelete, setConfirmDelete] = React.useState(false);

  const busy =
    !!activateBusy ||
    !!busyMap[`enSess:${session.id}`] ||
    !!busyMap[`delSess:${session.id}`] ||
    !!busyMap[`updSess:${session.id}`];

  return (
    <Card
      className={
        session.active
          ? "border-primary/60"
          : !session.enabled
            ? "opacity-60"
            : undefined
      }
    >
      <CardContent className="p-4">
        <div className="mb-2 flex items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="truncate font-medium">{session.name}</span>
              <Badge variant="outline">{kindLabel}</Badge>
              {session.active && <Badge variant="success">{t("active")}</Badge>}
              {!session.enabled && <Badge variant="secondary">{t("disabledLabel")}</Badge>}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2" title={t("enabledLabel")}>
            <span className="text-xs text-muted-foreground">
              {session.enabled ? t("enabledLabel") : t("disabledLabel")}
            </span>
            <Switch
              checked={session.enabled}
              disabled={busy}
              onCheckedChange={(v) => void setSessionEnabled(session.id, v)}
            />
          </div>
        </div>

        <div className="text-sm">
          {session.providerHint && (
            <KeyValue label={t("provider")} value={session.providerHint} />
          )}
          {session.accountHint && (
            <KeyValue label={t("account")} value={session.accountHint} mono />
          )}
          {extraRows?.map((r) => (
            <KeyValue key={r.label} label={r.label} value={r.value} mono={r.mono} />
          ))}
          <KeyValue label={t("lastUsed")} value={timeAgo(session.lastUsedAt)} />
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-2">
          {!session.active && (
            <Button
              size="sm"
              disabled={busy || !session.enabled}
              onClick={() => void onActivate()}
            >
              <Play /> {t("activate")}
            </Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              if (onEdit) {
                onEdit();
              } else {
                setNewName(session.name);
                setRenaming(true);
              }
            }}
          >
            <Pencil /> {onEdit ? t("edit") : t("rename")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="text-destructive hover:text-destructive"
            disabled={busy}
            onClick={() => setConfirmDelete(true)}
          >
            <Trash2 /> {t("delete")}
          </Button>
        </div>

        <Dialog open={renaming} onClose={() => setRenaming(false)} title={t("rename")}>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label>{t("apiKeyName")}</Label>
              <Input value={newName} onChange={(e) => setNewName(e.target.value)} />
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setRenaming(false)}>
                {t("cancel")}
              </Button>
              <Button
                disabled={!newName.trim()}
                onClick={async () => {
                  await updateSession(session.id, newName.trim());
                  setRenaming(false);
                }}
              >
                {t("save")}
              </Button>
            </div>
          </div>
        </Dialog>

        <ConfirmDialog
          open={confirmDelete}
          onClose={() => setConfirmDelete(false)}
          onConfirm={async () => {
            setConfirmDelete(false);
            await deleteSession(session.id);
          }}
          title={t("deleteConfirmTitle")}
          body={t("deleteConfirmBody")}
          destructive
          busy={busy}
        />
      </CardContent>
    </Card>
  );
}

export function DisableHarnessButton({
  disabled,
  busy,
  onClick,
}: {
  disabled?: boolean;
  busy?: boolean;
  onClick: () => Promise<void>;
}) {
  const t = useT();
  return (
    <Button
      variant="outline"
      size="sm"
      disabled={disabled || busy}
      className="text-destructive hover:text-destructive"
      onClick={() => void onClick()}
    >
      <PowerOff /> {t("disableBtn")}
    </Button>
  );
}
