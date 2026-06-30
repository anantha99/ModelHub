import { useEffect, useMemo, useRef, useState } from "react";
import { deleteModel, openPath, scanModels } from "../api/tauri";
import type { LocalModel, ModelSource, ScanResult, SourceStatus } from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { ModelCard, formatBytes, formatScanTimestamp } from "../components/ModelCard";

type LocalPageProps = {
  refreshReason?: "installed_download" | null;
  refreshSignal?: number;
};

type SourceGroup = {
  source: ModelSource;
  label: string;
  models: LocalModel[];
};

const sourceOrder: ModelSource[] = ["huggingface", "lmstudio", "ollama", "custom"];

const sourceLabels: Record<ModelSource, string> = {
  huggingface: "Hugging Face",
  lmstudio: "LM Studio",
  ollama: "Ollama",
  custom: "Custom folders",
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function groupModels(models: LocalModel[]): SourceGroup[] {
  return sourceOrder
    .map((source) => ({
      source,
      label: sourceLabels[source],
      models: models.filter((model) => model.source === source),
    }))
    .filter((group) => group.models.length > 0);
}

function sourceStatusTone(status: SourceStatus): "info" | "warning" | "error" {
  if (status.status === "error") {
    return "error";
  }

  if (status.status === "disabled") {
    return "info";
  }

  return "warning";
}

function visibleSourceStatuses(statuses: SourceStatus[]): SourceStatus[] {
  return statuses.filter((status) => status.status !== "ok" || status.message);
}

const installedDownloadRefreshMessage = "Installed download detected. Local library refreshed.";

type ActionBanner = {
  label: string;
  message: string;
  tone: "info" | "warning" | "error";
};

export function LocalPage({ refreshReason = null, refreshSignal = 0 }: LocalPageProps) {
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [refreshNotice, setRefreshNotice] = useState<string | null>(null);
  const [actionBanner, setActionBanner] = useState<ActionBanner | null>(null);
  const [deletingModelId, setDeletingModelId] = useState<string | null>(null);
  const isMounted = useRef(false);
  const latestRequestId = useRef(0);
  const lastHandledRefreshSignal = useRef(refreshSignal);
  const pendingInstalledRefresh = useRef(false);

  const modelGroups = useMemo(
    () => (scanResult ? groupModels(scanResult.models) : []),
    [scanResult],
  );
  const sourceNotices = useMemo(
    () => (scanResult ? visibleSourceStatuses(scanResult.sourceStatuses) : []),
    [scanResult],
  );
  const canRefresh = !isInitialLoading && !isRefreshing;

  async function runScan(
    mode: "initial" | "refresh",
    options: { force?: boolean; noticeOnSuccess?: string | null } = {},
  ) {
    if (mode === "refresh" && !options.force && !canRefresh) {
      return;
    }

    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;
    const hadResult = scanResult !== null;

    if (mode === "initial") {
      setIsInitialLoading(true);
      setError(null);
    } else {
      setIsRefreshing(true);
      setRefreshError(null);
    }

    setRefreshNotice(null);

    try {
      const result = await scanModels();

      if (!isMounted.current || requestId !== latestRequestId.current) {
        return;
      }

      setScanResult(result);
      setError(null);
      setRefreshError(null);
      setRefreshNotice(options.noticeOnSuccess ?? null);
    } catch (scanError) {
      if (!isMounted.current || requestId !== latestRequestId.current) {
        return;
      }

      const message = errorMessage(scanError);

      if (hadResult) {
        setRefreshError(`Refresh failed: ${message}`);
      } else {
        setError(message);
      }
      setRefreshNotice(null);
    } finally {
      if (!isMounted.current || requestId !== latestRequestId.current) {
        return;
      }

      setIsInitialLoading(false);
      setIsRefreshing(false);
    }
  }

  useEffect(() => {
    isMounted.current = true;
    void runScan("initial", {
      noticeOnSuccess:
        refreshSignal > 0 && refreshReason === "installed_download"
          ? installedDownloadRefreshMessage
          : null,
    });

    return () => {
      isMounted.current = false;
    };
  }, []);

  useEffect(() => {
    if (refreshSignal === 0 || refreshSignal === lastHandledRefreshSignal.current) {
      return;
    }

    lastHandledRefreshSignal.current = refreshSignal;

    if (refreshReason !== "installed_download") {
      return;
    }

    if (isInitialLoading || isRefreshing) {
      pendingInstalledRefresh.current = true;
      return;
    }

    void runScan("refresh", {
      force: true,
      noticeOnSuccess: installedDownloadRefreshMessage,
    });
  }, [refreshSignal, refreshReason, isInitialLoading, isRefreshing]);

  useEffect(() => {
    if (isInitialLoading || isRefreshing || !pendingInstalledRefresh.current) {
      return;
    }

    pendingInstalledRefresh.current = false;
    void runScan("refresh", {
      force: true,
      noticeOnSuccess: installedDownloadRefreshMessage,
    });
  }, [isInitialLoading, isRefreshing]);

  async function copyText(label: string, value: string | null) {
    if (!value) {
      setActionBanner({
        label,
        message: "Nothing is available to copy for this model.",
        tone: "warning",
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(value);
      setActionBanner({
        label,
        message: "Copied to clipboard.",
        tone: "info",
      });
    } catch (copyError) {
      setActionBanner({
        label,
        message: `Copy failed: ${errorMessage(copyError)}`,
        tone: "error",
      });
    }
  }

  async function handleOpenPath(model: LocalModel) {
    if (!model.path) {
      return;
    }

    try {
      await openPath(model.path);
      setActionBanner({
        label: "Open folder",
        message: `Opened ${model.displayName}.`,
        tone: "info",
      });
    } catch (openError) {
      setActionBanner({
        label: "Open folder",
        message: errorMessage(openError),
        tone: "error",
      });
    }
  }

  async function handleDelete(model: LocalModel) {
    if (!model.path || model.source === "ollama") {
      return;
    }

    const confirmed = window.confirm(
      `Move "${model.displayName}" to the Recycle Bin?\n\n${model.path}`,
    );

    if (!confirmed) {
      return;
    }

    setDeletingModelId(model.id);
    setActionBanner(null);

    try {
      const result = await deleteModel({
        id: model.id,
        source: model.source,
        path: model.path,
        repoId: model.repoId,
      });

      setActionBanner({
        label: "Delete",
        message: `${result.message} ${result.deletedPath}`,
        tone: "info",
      });
      void runScan("refresh", { force: true });
    } catch (deleteError) {
      setActionBanner({
        label: "Delete failed",
        message: errorMessage(deleteError),
        tone: "error",
      });
    } finally {
      setDeletingModelId(null);
    }
  }

  return (
    <div className="page-stack local-page">
      <header className="page-header local-hero">
        <div>
          <p className="eyebrow">Local library</p>
          <h2>Installed models</h2>
          <p>
            Scan Hugging Face cache, LM Studio, and Ollama sources from one local
            Windows inventory.
          </p>
        </div>
        <div className="local-summary-panel" aria-live="polite">
          <span className="summary-label">Models</span>
          <strong>{scanResult?.models.length ?? "--"}</strong>
          <span>{formatBytes(scanResult?.totalSizeBytes ?? null)}</span>
        </div>
        <div className="local-hero-actions">
          <span className="scan-timestamp">
            {scanResult ? `Scanned ${formatScanTimestamp(scanResult.scannedAt)}` : "Not scanned yet"}
          </span>
          <button
            className="primary-button"
            disabled={!canRefresh}
            onClick={() => void runScan(scanResult ? "refresh" : "initial")}
            type="button"
          >
            {isRefreshing || isInitialLoading ? "Scanning..." : "Refresh"}
          </button>
        </div>
      </header>

      {refreshError ? <StatusBanner tone="error" label="Refresh" message={refreshError} /> : null}
      {refreshNotice ? <StatusBanner tone="info" label="Library" message={refreshNotice} /> : null}
      {actionBanner ? (
        <StatusBanner tone={actionBanner.tone} label={actionBanner.label} message={actionBanner.message} />
      ) : null}

      {sourceNotices.length > 0 ? (
        <section className="source-notice-list" aria-label="Source scan notices">
          {sourceNotices.map((status) => (
            <SourceStatusNotice key={`${status.source}-${status.status}-${status.path ?? "none"}`} status={status} />
          ))}
        </section>
      ) : null}

      {isInitialLoading ? (
        <section className="settings-card local-loading-card">
          <p className="eyebrow">Scanning</p>
          <h3>Checking local model sources</h3>
          <p>
            ModelHub is reading configured model roots without blocking the app
            window.
          </p>
        </section>
      ) : error ? (
        <section className="settings-card local-loading-card">
          <p className="eyebrow">Scan failed</p>
          <h3>Local models could not be loaded</h3>
          <p>{error}</p>
          <button className="secondary-button" onClick={() => void runScan("initial")} type="button">
            Try Again
          </button>
        </section>
      ) : scanResult && scanResult.models.length > 0 ? (
        <div className="model-group-list">
          {modelGroups.map((group) => (
            <section className="model-source-group" key={group.source}>
              <div className="section-heading-row">
                <div>
                  <p className="eyebrow">{group.label}</p>
                  <h3>{group.models.length} local {group.models.length === 1 ? "model" : "models"}</h3>
                </div>
                <span className="soft-pill">{group.source}</span>
              </div>
              <div className="model-card-grid">
                {group.models.map((model) => (
                  <ModelCard
                    deleting={deletingModelId === model.id}
                    key={model.id}
                    model={model}
                    onCopyId={(selectedModel) => void copyText("Copy ID", selectedModel.id)}
                    onCopyPath={(selectedModel) => void copyText("Copy path", selectedModel.path)}
                    onDelete={(selectedModel) => void handleDelete(selectedModel)}
                    onOpenPath={(selectedModel) => void handleOpenPath(selectedModel)}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <EmptyState
          eyebrow="No models found"
          title="Your local library is empty"
          description="No configured source returned a model. Check Settings if a model folder is missing, then refresh this page."
        />
      )}
    </div>
  );
}

function StatusBanner({
  label,
  message,
  tone,
}: {
  label: string;
  message: string;
  tone: "info" | "warning" | "error";
}) {
  return (
    <section className="status-banner" data-tone={tone}>
      <strong>{label}</strong>
      <span>{message}</span>
    </section>
  );
}

function SourceStatusNotice({ status }: { status: SourceStatus }) {
  const sourceLabel = sourceLabels[status.source];
  const tone = sourceStatusTone(status);
  const message = status.message ?? sourceStatusFallback(status);

  return (
    <StatusBanner
      label={sourceLabel}
      message={status.path ? `${message} ${status.path}` : message}
      tone={tone}
    />
  );
}

function sourceStatusFallback(status: SourceStatus): string {
  if (status.status === "missing") {
    return "Source folder is missing.";
  }

  if (status.status === "disabled") {
    return "Source is not running or not configured.";
  }

  if (status.status === "error") {
    return "Source could not be scanned.";
  }

  return "Source scanned with warnings.";
}
