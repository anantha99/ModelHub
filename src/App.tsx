import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { AppShell } from "./components/AppShell";
import { DownloadsPage } from "./pages/DownloadsPage";
import { ExplorePage } from "./pages/ExplorePage";
import { LocalPage } from "./pages/LocalPage";
import { RuntimesPage } from "./pages/RuntimesPage";
import { SettingsPage } from "./pages/SettingsPage";
import type { AppPage, DownloadJob, TrayNavigatePayload } from "./api/types";

const appPages = new Set<AppPage>(["local", "explore", "downloads", "runtimes", "settings"]);

function isTrayNavigatePayload(payload: unknown): payload is TrayNavigatePayload {
  return (
    typeof payload === "object" &&
    payload !== null &&
    "page" in payload &&
    typeof payload.page === "string" &&
    appPages.has(payload.page as AppPage)
  );
}

function isInstalledDownload(job: DownloadJob): boolean {
  return job.installedAt !== null && job.snapshotPath !== null;
}

function App() {
  const [activePage, setActivePage] = useState<AppPage>("local");
  const [localRefreshSignal, setLocalRefreshSignal] = useState(0);
  const [settingsHasUnsavedChanges, setSettingsHasUnsavedChanges] = useState(false);
  const installedJobIds = useRef<Set<string>>(new Set());

  const requestPageChange = useCallback(
    (page: AppPage) => {
      if (page === activePage) {
        return;
      }

      if (activePage === "settings" && settingsHasUnsavedChanges) {
        const confirmed = window.confirm(
          "Leave Settings and discard unsaved changes?",
        );

        if (!confirmed) {
          return;
        }

        setSettingsHasUnsavedChanges(false);
      }

      setActivePage(page);
    },
    [activePage, settingsHasUnsavedChanges],
  );

  useEffect(() => {
    const unlisten = listen<unknown>("tray:navigate", (event) => {
      if (isTrayNavigatePayload(event.payload)) {
        requestPageChange(event.payload.page);
      }
    });

    return () => {
      unlisten.then((cleanup) => cleanup());
    };
  }, [requestPageChange]);

  useEffect(() => {
    const unlisten = listen<DownloadJob>("download:updated", (event) => {
      const job = event.payload;

      if (!isInstalledDownload(job) || installedJobIds.current.has(job.id)) {
        return;
      }

      installedJobIds.current.add(job.id);
      setLocalRefreshSignal((current) => current + 1);
    });

    return () => {
      unlisten.then((cleanup) => cleanup());
    };
  }, []);

  function renderPage(): ReactNode {
    switch (activePage) {
      case "local":
        return (
          <LocalPage
            refreshReason={localRefreshSignal > 0 ? "installed_download" : null}
            refreshSignal={localRefreshSignal}
          />
        );
      case "explore":
        return <ExplorePage />;
      case "downloads":
        return <DownloadsPage />;
      case "runtimes":
        return <RuntimesPage />;
      case "settings":
        return <SettingsPage onDirtyChange={setSettingsHasUnsavedChanges} />;
    }
  }

  return (
    <AppShell activePage={activePage} onNavigate={requestPageChange}>
      {renderPage()}
    </AppShell>
  );
}

export default App;
