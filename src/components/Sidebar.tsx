import { useEffect, useState } from "react";
import { getSystemInfo } from "../api/tauri";
import type { AppPage, DiskInfo, GpuInfo, NavigationItem, SystemInfo } from "../api/types";
import { formatBytes } from "../utils/format";

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
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [systemInfoError, setSystemInfoError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    getSystemInfo()
      .then((info) => {
        if (cancelled) {
          return;
        }

        setSystemInfo(info);
        setSystemInfoError(null);
      })
      .catch(() => {
        if (cancelled) {
          return;
        }

        setSystemInfoError("System info unavailable");
      });

    return () => {
      cancelled = true;
    };
  }, []);

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
              aria-current={isActive ? "page" : undefined}
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

      <SystemInfoCard error={systemInfoError} systemInfo={systemInfo} />

      <div className="sidebar-footer">
        <span className="status-dot" aria-hidden="true" />
        <span>App shell ready</span>
      </div>
    </aside>
  );
}

function SystemInfoCard({
  error,
  systemInfo,
}: {
  error: string | null;
  systemInfo: SystemInfo | null;
}) {
  const rows = systemInfo
    ? [
        { label: "CPU", value: systemInfo.cpu?.name ?? "Unavailable" },
        { label: "RAM", value: formatBytes(systemInfo.memory.totalBytes, "Unavailable") },
        { label: "GPU", value: formatGpu(systemInfo.gpus) },
        { label: "Disk", value: formatDisk(systemInfo.hfCacheDisk) },
      ]
    : [{ label: "Status", value: error ?? "Loading" }];

  return (
    <section className="system-info-card" aria-label="System information" aria-live="polite">
      <h2>System</h2>
      <dl>
        {rows.map((row) => (
          <div className="system-info-row" key={row.label}>
            <dt>{row.label}</dt>
            <dd title={row.value}>{row.value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function formatGpu(gpus: GpuInfo[]): string {
  const gpu = gpus[0];

  if (!gpu) {
    return "Unavailable";
  }

  const memory = gpu.memoryBytes === null ? "" : ` (${formatBytes(gpu.memoryBytes, "Unknown")})`;

  return `${gpu.name}${memory}`;
}

function formatDisk(disk: DiskInfo | null): string {
  if (!disk) {
    return "Unavailable";
  }

  return `${formatBytes(disk.availableBytes, "Unknown")} free of ${formatBytes(disk.totalBytes, "Unknown")}`;
}
