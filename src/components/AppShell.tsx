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
      <a className="skip-link" href="#main-content">
        Skip to main content
      </a>
      <Sidebar activePage={activePage} onNavigate={onNavigate} />
      <main className="main-panel" id="main-content" tabIndex={-1}>
        {children}
      </main>
    </div>
  );
}
