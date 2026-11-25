// Simplified 3-tab AppShell focused on harnesses, accounts, and settings.

import {
  Cpu,
  KeyRound,
  Settings as SettingsIcon,
  ShieldCheck,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { StatusDot } from "@/components/common";
import { useApp, useT, type PageId } from "@/stores/app";

const NAV: Array<{
  id: PageId;
  icon: React.ReactNode;
  key: Parameters<ReturnType<typeof useT>>[0];
}> = [
  { id: "harnesses", icon: <Cpu className="h-4 w-4" />, key: "navHarnesses" },
  { id: "accounts", icon: <KeyRound className="h-4 w-4" />, key: "navAccounts" },
  { id: "settings", icon: <SettingsIcon className="h-4 w-4" />, key: "navSettings" },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const t = useT();
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const state = useApp((s) => s.state);
  const proxy = state?.proxy;

  const accountCount = state?.accounts.length ?? 0;

  let activeHarnesses = 0;
  if (state) {
    if (state.claudeLive.detectedMode !== "disabled") activeHarnesses += 1;
    if (state.opencodeLive.configSource !== "none") activeHarnesses += 1;
    if (state.zcodeLive.v2Exists && (state.zcodeLive.activePersonalName || state.zcodeLive.zaiMode === "oauth")) {
      activeHarnesses += 1;
    }
    if (state.antigravityLive.hasKeyringToken) activeHarnesses += 1;
  }

  const proxyRunning = !!proxy?.processAlive && !!proxy?.serverRunning;

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <aside className="flex w-60 shrink-0 flex-col border-r bg-card">
        <div className="flex items-center gap-3 px-5 pb-4 pt-5">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-br from-cyan-500 to-indigo-600 text-white shadow-sm">
            <ShieldCheck className="h-5 w-5" />
          </div>
          <div>
            <div className="text-sm font-semibold leading-tight">{t("appName")}</div>
            <div className="text-[11px] text-muted-foreground">{t("appTagline")}</div>
          </div>
        </div>

        <nav className="flex-1 space-y-1 px-3 pt-1">
          {NAV.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setPage(item.id)}
              className={cn(
                "flex w-full items-center justify-between rounded-md px-3 py-2 text-sm font-medium transition-colors",
                page === item.id
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
              )}
            >
              <span className="flex items-center gap-3">
                {item.icon}
                <span className="truncate">{t(item.key)}</span>
              </span>
              {item.id === "accounts" && accountCount > 0 && (
                <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] font-normal text-muted-foreground">
                  {accountCount}
                </span>
              )}
            </button>
          ))}
        </nav>

        <div className="border-t px-5 py-3 space-y-1.5">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <StatusDot tone={activeHarnesses > 0 ? "running" : "stopped"} />
            <span className="truncate">
              {activeHarnesses} / 4 {t("harnessesActiveLabel")}
            </span>
          </div>
          {proxyRunning && (
            <div className="pl-4 text-[11px] text-muted-foreground/75">
              {t("proxyActiveBadge")}
            </div>
          )}
        </div>
      </aside>

      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-6xl px-8 py-7">{children}</div>
      </main>
    </div>
  );
}
