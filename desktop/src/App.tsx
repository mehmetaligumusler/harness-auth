import * as React from "react";
import { listen } from "@tauri-apps/api/event";
import { Toaster, toast } from "sonner";
import { AppShell } from "@/components/AppShell";
import { useApp } from "@/stores/app";
import { translate } from "@/lib/i18n";
import type { HarnessOAuthDone, OAuthDone, ProxyStatusDto } from "@/lib/types";
import { HarnessesPage } from "@/pages/HarnessesPage";
import { AccountsPage } from "@/pages/AccountsPage";
import { SettingsPage } from "@/pages/SettingsPage";

export default function App() {
  const page = useApp((s) => s.page);
  const refresh = useApp((s) => s.refresh);
  const theme = useApp((s) => s.state?.settings.theme ?? "dark");

  React.useEffect(() => {
    void refresh();
    const unprox = listen<ProxyStatusDto>("proxy://state", (e) => {
      const cur = useApp.getState().state;
      if (cur) useApp.setState({ state: { ...cur, proxy: e.payload } });
    });
    const unurl = listen<string>("oauth://url", (e) => {
      useApp.setState({ oauthUrl: e.payload });
    });
    const undone = listen<OAuthDone>("oauth://done", (e) => {
      const lang = useApp.getState().state?.settings.language ?? "tr";
      useApp.setState({ oauthUrl: null });
      if (e.payload.ok) {
        toast.success(translate(lang, "oauthDoneOk"));
      } else {
        toast.error(translate(lang, "oauthDoneFail"), {
          description: e.payload.error ?? "",
        });
      }
      void useApp.getState().refresh();
    });
    const unharness = listen<HarnessOAuthDone>("harness-oauth://done", (e) => {
      const lang = useApp.getState().state?.settings.language ?? "tr";
      if (e.payload.ok) {
        toast.success(translate(lang, "harnessOAuthDoneOk"));
      } else {
        toast.error(translate(lang, "harnessOAuthDoneFail"), {
          description: e.payload.error ?? "",
        });
      }
      void useApp.getState().refresh();
    });
    return () => {
      void unprox.then((f) => f());
      void unurl.then((f) => f());
      void undone.then((f) => f());
      void unharness.then((f) => f());
    };
  }, [refresh]);

  React.useEffect(() => {
    const id = setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, 3000);
    return () => clearInterval(id);
  }, [refresh]);

  React.useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  return (
    <>
      <AppShell>
        {page === "harnesses" && <HarnessesPage />}
        {page === "accounts" && <AccountsPage />}
        {page === "settings" && <SettingsPage />}
      </AppShell>
      <Toaster position="bottom-right" richColors closeButton />
    </>
  );
}
