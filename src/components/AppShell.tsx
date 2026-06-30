import type { ReactNode } from "react";
import { Sidebar } from "./Sidebar";
import type { AppPage } from "../api/types";

type AppShellProps = {
  activePage: AppPage;
  onNavigate: (page: AppPage) => void;
  children: ReactNode;
};

export function AppShell({ activePage, onNavigate, children }: AppShellProps) {
  return (
    <div className="app-shell">
      <Sidebar activePage={activePage} onNavigate={onNavigate} />
      <main className="main-panel" aria-live="polite">
        {children}
      </main>
    </div>
  );
}
