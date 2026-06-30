import type { AppPage, NavigationItem } from "../api/types";

const navigationItems: NavigationItem[] = [
  {
    id: "local",
    label: "Local",
    description: "Installed and discovered models",
  },
  {
    id: "explore",
    label: "Explore",
    description: "Search Hugging Face",
  },
  {
    id: "downloads",
    label: "Downloads",
    description: "Active and completed jobs",
  },
  {
    id: "runtimes",
    label: "Runtimes",
    description: "Ollama and LM Studio status",
  },
  {
    id: "settings",
    label: "Settings",
    description: "Paths and app behavior",
  },
];

type SidebarProps = {
  activePage: AppPage;
  onNavigate: (page: AppPage) => void;
};

export function Sidebar({ activePage, onNavigate }: SidebarProps) {
  return (
    <aside className="sidebar" aria-label="Primary navigation">
      <div className="brand-block">
        <span className="brand-mark" aria-hidden="true">
          MH
        </span>
        <div>
          <h1>ModelHub</h1>
          <p>Windows model manager</p>
        </div>
      </div>

      <nav className="nav-list">
        {navigationItems.map((item) => {
          const isActive = item.id === activePage;

          return (
            <button
              className="nav-item"
              data-active={isActive}
              key={item.id}
              onClick={() => onNavigate(item.id)}
              type="button"
            >
              <span>{item.label}</span>
              <small>{item.description}</small>
            </button>
          );
        })}
      </nav>

      <div className="sidebar-footer">
        <span className="status-dot" aria-hidden="true" />
        <span>App shell ready</span>
      </div>
    </aside>
  );
}
