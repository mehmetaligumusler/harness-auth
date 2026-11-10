// Shared app-level components.

import * as React from "react";
import { Check, Copy, Inbox } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button, Dialog } from "@/components/ui";
import { useT } from "@/stores/app";

export function StatusDot({ tone }: { tone: "running" | "stopped" | "warn" }) {
  const color =
    tone === "running"
      ? "bg-success"
      : tone === "warn"
        ? "bg-warning"
        : "bg-muted-foreground/40";
  return (
    <span className="relative inline-flex h-2.5 w-2.5">
      {tone === "running" && (
        <span className={cn("absolute inline-flex h-full w-full animate-ping rounded-full opacity-60", color)} />
      )}
      <span className={cn("relative inline-flex h-2.5 w-2.5 rounded-full", color)} />
    </span>
  );
}

export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}) {
  return (
    <div className="mb-6 flex items-start justify-between gap-4">
      <div>
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

export function EmptyState({
  title,
  description,
  action,
  icon,
}: {
  title: string;
  description?: string;
  action?: React.ReactNode;
  icon?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-dashed px-6 py-14 text-center">
      <div className="mb-3 text-muted-foreground/60">{icon ?? <Inbox className="h-10 w-10" />}</div>
      <div className="font-medium">{title}</div>
      {description && (
        <p className="mt-1 max-w-sm text-sm text-muted-foreground">{description}</p>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

export function CopyField({ value, mask }: { value: string; mask?: boolean }) {
  const t = useT();
  const [copied, setCopied] = React.useState(false);
  const [revealed, setRevealed] = React.useState(!mask);
  const display = revealed ? value : `${value.slice(0, 6)}${"•".repeat(Math.max(4, value.length - 10))}${value.slice(-4)}`;
  return (
    <div className="flex items-center gap-1.5">
      <code className="min-w-0 flex-1 truncate rounded-md border bg-muted/50 px-2 py-1.5 font-mono text-xs">
        {display}
      </code>
      {mask && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setRevealed((r) => !r)}
          className="h-8 px-2 text-xs"
        >
          {revealed ? "Gizle" : "Göster"}
        </Button>
      )}
      <Button
        variant="ghost"
        size="icon"
        className="h-8 w-8"
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(value);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          } catch {
            /* clipboard unavailable */
          }
        }}
        aria-label={t("copy")}
      >
        {copied ? <Check className="h-4 w-4 text-success" /> : <Copy className="h-4 w-4" />}
      </Button>
    </div>
  );
}

export function KeyValue({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex min-w-0 items-baseline justify-between gap-3 py-1.5">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      <span className={cn("min-w-0 truncate text-sm font-medium", mono && "font-mono text-xs")}>
        {value}
      </span>
    </div>
  );
}

export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  body,
  destructive,
  busy,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  body: string;
  destructive?: boolean;
  busy?: boolean;
}) {
  const t = useT();
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      description={body}
      footer={
        <>
          <Button variant="outline" onClick={onClose} disabled={busy}>
            {t("cancel")}
          </Button>
          <Button
            variant={destructive ? "destructive" : "default"}
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? t("loading") : t("confirm")}
          </Button>
        </>
      }
    />
  );
}

/** Section header inside cards. */
export function SectionTitle({ children }: { children: React.ReactNode }) {
  return <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{children}</div>;
}
