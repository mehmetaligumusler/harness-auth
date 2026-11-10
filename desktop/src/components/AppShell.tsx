// App shell: sidebar navigation + main content area.

import {
  Bot,
  Gauge,
  KeyRound,
  Layers,
  Monitor,
  Notebook,
  Settings as SettingsIcon,
  Workflow,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { StatusDot } from "@/components/common";
import { useApp, useT, type PageId } from "@/stores/app";

const NAV: Array<{ id: PageId; icon: React.ReactNode; key: Parameters<ReturnType<typeof useT>>[0] }> = [
  { id: "dashboard", icon: <Gauge className="h-4 w-4" />, key: "navDashboard" },
  { id: "accounts", icon: <KeyRound className="h-4 w-4" />, key: "navAccounts" },
  { id: "plans", icon: <Layers className="h-4 w-4" />, key: "navPlans" },
  { id: "models", icon: <Bot className="h-4 w-4" />, key: "navModels" },
  { id: "claude", icon: <Monitor className="h-4 w-4" />, key: "navClaude" },
  { id: "proxy", icon: <Workflow className="h-4 w-4" />, key: "navProxy" },
  { id: "logs", icon: <Notebook className="h-4 w-4" />, key: "navLogs" },
  { id: "settings", icon: <SettingsIcon className="h-4 w-4" />, key: "navSettings" },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const t = useT();
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const proxy = useApp((s) => s.state?.proxy);

  const running = !!proxy?.processAlive && !!proxy?.serverRunning;

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <aside className="flex w-60 shrink-0 flex-col border-r bg-card">
        <div className="flex items-center gap-2.5 px-5 pb-4 pt-5">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Bot className="h-5 w-5" />
          </div>
          <div>
            <div className="text-sm font-semibold leading-tight">{t("appName")}</div>
            <div className="text-[11px] text-muted-foreground">ZCode Proxy</div>
          </div>
        </div>

        <nav className="flex-1 space-y-0.5 px-3">
          {NAV.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setPage(item.id)}
              className={cn(
                "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                page === item.id
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
              )}
            >
              {item.icon}
              <span className="truncate">{t(item.key)}</span>
            </button>
          ))}
        </nav>

        <div className="border-t px-5 py-3">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <StatusDot tone={running ? "running" : proxy?.processAlive ? "warn" : "stopped"} />
            <span>
              {running
                ? t("statusRunning")
                : proxy?.processAlive
                  ? t("statusServerStopped")
                  : t("statusProcessDown")}
            </span>
          </div>
        </div>
      </aside>

      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-6xl px-8 py-7">{children}</div>
      </main>
    </div>
  );
}
