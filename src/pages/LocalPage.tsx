import { useEffect, useMemo, useRef, useState } from "react";
import { deleteModel, openPath, scanModels } from "../api/tauri";
import type { LocalModel, ModelFormat, ModelSource, ScanResult, SourceStatus } from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { formatBytes, formatRelativeTime, formatScanTimestamp } from "../utils/format";

type LocalPageProps = {
  refreshReason?: "installed_download" | null;
  refreshSignal?: number;
};

type LocalSourceFilter = ModelSource | "all";
type LocalSortKey = "model" | "source" | "files" | "size" | "format" | "quantization" | "modified";
type SortDirection = "asc" | "desc";

type ActionBanner = {
  label: string;
  message: string;
  tone: "info" | "warning" | "error";
};

const pageSize = 8;
const sourceOrder: ModelSource[] = ["huggingface", "lmstudio", "ollama", "custom"];
const installedDownloadRefreshMessage = "Installed download detected. Local library refreshed.";

const sourceLabels: Record<ModelSource, string> = {
  huggingface: "Hugging Face",
  lmstudio: "LM Studio",
  ollama: "Ollama",
  custom: "Custom",
};

const sourceShortLabels: Record<ModelSource, string> = {
  huggingface: "HF",
  lmstudio: "LM",
  ollama: "OL",
  custom: "CU",
};

const formatLabels: Record<ModelFormat, string> = {
  gguf: "GGUF",
  safetensors: "Safetensors",
  onnx: "ONNX",
  mlx: "MLX",
  unknown: "Unknown",
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function visibleSourceStatuses(statuses: SourceStatus[]): SourceStatus[] {
  return statuses.filter((status) => status.status !== "ok" || status.message);
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

export function LocalPage({ refreshReason = null, refreshSignal = 0 }: LocalPageProps) {
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [refreshNotice, setRefreshNotice] = useState<string | null>(null);
  const [actionBanner, setActionBanner] = useState<ActionBanner | null>(null);
  const [deletingModelId, setDeletingModelId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sourceFilter, setSourceFilter] = useState<LocalSourceFilter>("all");
  const [sortKey, setSortKey] = useState<LocalSortKey>("modified");
  const [sortDirection, setSortDirection] = useState<SortDirection>("desc");
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const isMounted = useRef(false);
  const latestRequestId = useRef(0);
  const lastHandledRefreshSignal = useRef(refreshSignal);
  const pendingInstalledRefresh = useRef(false);

  const models = scanResult?.models ?? [];
  const sourceNotices = useMemo(
    () => (scanResult ? visibleSourceStatuses(scanResult.sourceStatuses) : []),
    [scanResult],
  );
  const sourceCounts = useMemo(() => countSources(models), [models]);
  const filteredModels = useMemo(
    () => sortModels(filterModels(models, sourceFilter, query), sortKey, sortDirection),
    [models, query, sourceFilter, sortDirection, sortKey],
  );
  const pageCount = Math.max(1, Math.ceil(filteredModels.length / pageSize));
  const currentPage = Math.min(page, pageCount);
  const pagedModels = filteredModels.slice((currentPage - 1) * pageSize, currentPage * pageSize);
  const selectedModel = filteredModels.find((model) => model.id === selectedModelId) ?? filteredModels[0] ?? null;
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

  useEffect(() => {
    setPage(1);
  }, [query, sourceFilter, sortDirection, sortKey]);

  useEffect(() => {
    if (page > pageCount) {
      setPage(pageCount);
    }
  }, [page, pageCount]);

  useEffect(() => {
    if (!selectedModelId || !filteredModels.some((model) => model.id === selectedModelId)) {
      setSelectedModelId(filteredModels[0]?.id ?? null);
    }
  }, [filteredModels, selectedModelId]);

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
      setActionBanner({ label, message: "Copied to clipboard.", tone: "info" });
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
      setActionBanner({ label: "Open folder", message: `Opened ${model.displayName}.`, tone: "info" });
    } catch (openError) {
      setActionBanner({ label: "Open folder", message: errorMessage(openError), tone: "error" });
    }
  }

  async function handleDelete(model: LocalModel) {
    if (!model.path || model.source === "ollama") {
      return;
    }

    const confirmed = window.confirm(`Move "${model.displayName}" to the Recycle Bin?\n\n${model.path}`);

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
      setActionBanner({ label: "Delete failed", message: errorMessage(deleteError), tone: "error" });
    } finally {
      setDeletingModelId(null);
    }
  }

  function toggleSort(nextSortKey: LocalSortKey) {
    if (sortKey === nextSortKey) {
      setSortDirection((current) => (current === "asc" ? "desc" : "asc"));
      return;
    }

    setSortKey(nextSortKey);
    setSortDirection(nextSortKey === "model" ? "asc" : "desc");
  }

  function handlePageChange(nextPage: number) {
    const boundedPage = Math.min(Math.max(nextPage, 1), pageCount);

    setPage(boundedPage);
    setSelectedModelId(filteredModels[(boundedPage - 1) * pageSize]?.id ?? null);
  }

  return (
    <div className="page-stack local-page local-library-page">
      <header className="local-library-header">
        <div>
          <h2>Local Models</h2>
          <p>All models on your machine</p>
        </div>
        <div className="local-library-toolbar">
          <label className="local-search-field" htmlFor="local-model-search">
            <span className="sr-only">Search models</span>
            <input
              autoComplete="off"
              id="local-model-search"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search models..."
              type="search"
              value={query}
            />
          </label>
          <button className="secondary-button local-icon-button" type="button" aria-label="More local model actions">
            ...
          </button>
        </div>
      </header>

      {refreshError ? <StatusBanner tone="error" label="Refresh" message={refreshError} /> : null}
      {refreshNotice ? <StatusBanner tone="info" label="Library" message={refreshNotice} /> : null}
      {actionBanner ? <StatusBanner tone={actionBanner.tone} label={actionBanner.label} message={actionBanner.message} /> : null}

      {isInitialLoading ? (
        <section className="settings-card local-loading-card">
          <p className="eyebrow">Scanning</p>
          <h3>Checking local model sources</h3>
          <p>ModelHub is reading configured model roots without blocking the app window.</p>
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
      ) : scanResult && models.length > 0 ? (
        <>
          <section className="local-stat-grid" aria-label="Local model summary">
            <LocalStatCard label="All Models" value={String(models.length)} detail={`Across ${activeSourceCount(sourceCounts)} sources`} />
            <LocalStatCard label="Disk Used" value={formatBytes(scanResult.totalSizeBytes, "Unknown")} detail={`In ${models.length} models`} />
            <LocalStatCard label="GGUF Models" value={String(models.filter((model) => model.format === "gguf").length)} detail="Most common local format" />
            <LocalStatCard label="Last Scan" value={formatRelativeTime(scanResult.scannedAt)} detail="Click rescan to refresh" />
          </section>

          <SourceTabs counts={sourceCounts} selected={sourceFilter} onSelect={setSourceFilter} />

          {sourceNotices.length > 0 ? (
            <section className="source-notice-list" aria-label="Source scan notices">
              {sourceNotices.map((status) => (
                <SourceStatusNotice key={`${status.source}-${status.status}-${status.path ?? "none"}`} status={status} />
              ))}
            </section>
          ) : null}

          <section className="local-library-layout">
            <div className="local-model-list-panel">
              <ModelTable
                models={pagedModels}
                onSelect={setSelectedModelId}
                onSort={toggleSort}
                selectedModelId={selectedModel?.id ?? null}
                sortDirection={sortDirection}
                sortKey={sortKey}
              />
              <div className="local-table-footer">
                <span>{paginationLabel(filteredModels.length, currentPage)}</span>
                <Pagination currentPage={currentPage} pageCount={pageCount} onPageChange={handlePageChange} />
              </div>
            </div>

            <LocalModelDetailPanel
              deleting={selectedModel ? deletingModelId === selectedModel.id : false}
              model={selectedModel}
              onCopyId={(model) => void copyText("Copy model ID", model.repoId ?? model.id)}
              onCopyPath={(model) => void copyText("Copy path", model.path)}
              onDelete={(model) => void handleDelete(model)}
              onOpenPath={(model) => void handleOpenPath(model)}
            />
          </section>

          <footer className="local-library-footer">
            <span className="local-source-health">
              <span className="status-dot" aria-hidden="true" />
              Models loaded from {activeSourceCount(sourceCounts)} sources
            </span>
            <button className="primary-button" disabled={!canRefresh} onClick={() => void runScan("refresh")} type="button">
              {isRefreshing ? "Rescanning..." : "Rescan All Sources"}
            </button>
          </footer>
        </>
      ) : (
        <>
          {sourceNotices.length > 0 ? (
            <section className="source-notice-list" aria-label="Source scan notices">
              {sourceNotices.map((status) => (
                <SourceStatusNotice key={`${status.source}-${status.status}-${status.path ?? "none"}`} status={status} />
              ))}
            </section>
          ) : null}

          <EmptyState
            eyebrow="No models found"
            title="Your local library is empty"
            description="No configured source returned a model. Check Settings if a model folder is missing, then refresh this page."
          />

          {scanResult ? (
            <footer className="local-library-footer">
              <span className="local-source-health">
                <span className="status-dot" aria-hidden="true" />
                No local models found
              </span>
              <button className="primary-button" disabled={!canRefresh} onClick={() => void runScan("refresh")} type="button">
                {isRefreshing ? "Rescanning..." : "Rescan All Sources"}
              </button>
            </footer>
          ) : null}
        </>
      )}
    </div>
  );
}

function LocalStatCard({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <article className="local-stat-card">
      <span className="local-stat-icon" aria-hidden="true">
        {label.slice(0, 1)}
      </span>
      <div>
        <span>{label}</span>
        <strong>{value}</strong>
        <small>{detail}</small>
      </div>
    </article>
  );
}

function SourceTabs({
  counts,
  selected,
  onSelect,
}: {
  counts: Record<ModelSource, number>;
  selected: LocalSourceFilter;
  onSelect: (source: LocalSourceFilter) => void;
}) {
  const total = sourceOrder.reduce((count, source) => count + counts[source], 0);

  return (
    <div className="local-source-tabs" aria-label="Filter local models by source">
      <button data-active={selected === "all"} onClick={() => onSelect("all")} type="button">
        <span>All Sources</span>
        <strong>{total}</strong>
      </button>
      {sourceOrder.map((source) => (
        <button data-active={selected === source} key={source} onClick={() => onSelect(source)} type="button">
          <span>{sourceLabels[source]}</span>
          <strong>{counts[source]}</strong>
        </button>
      ))}
    </div>
  );
}

function ModelTable({
  models,
  onSelect,
  onSort,
  selectedModelId,
  sortDirection,
  sortKey,
}: {
  models: LocalModel[];
  onSelect: (modelId: string) => void;
  onSort: (sortKey: LocalSortKey) => void;
  selectedModelId: string | null;
  sortDirection: SortDirection;
  sortKey: LocalSortKey;
}) {
  return (
    <div className="local-model-table" role="table" aria-label="Local models">
      <div className="local-model-table-head" role="row">
        <SortHeader label="Model" sortKey="model" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <SortHeader label="Source" sortKey="source" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <SortHeader label="Files" sortKey="files" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <SortHeader label="Size" sortKey="size" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <SortHeader label="Format" sortKey="format" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <SortHeader label="Quantization" sortKey="quantization" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <SortHeader label="Modified" sortKey="modified" activeSortKey={sortKey} direction={sortDirection} onSort={onSort} />
        <span aria-hidden="true" />
      </div>
      {models.map((model) => (
        <div
          className="local-model-row"
          data-selected={model.id === selectedModelId}
          key={model.id}
          onClick={() => onSelect(model.id)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              onSelect(model.id);
            }
          }}
          role="row"
          tabIndex={0}
        >
          <div className="local-model-name-cell" role="cell">
            <span className="local-source-mark" data-source={model.source} aria-hidden="true">
              {sourceShortLabels[model.source]}
            </span>
            <span>
              <strong>{model.displayName}</strong>
              <small>{model.repoId ?? model.provider ?? model.id}</small>
            </span>
          </div>
          <span role="cell">{sourceLabels[model.source]}</span>
          <span role="cell">{model.files.length || "-"}</span>
          <span role="cell">{formatBytes(model.sizeBytes, "Unknown")}</span>
          <span role="cell">{formatModelFormat(model.format)}</span>
          <span role="cell">{model.quantization ?? "-"}</span>
          <span role="cell">{formatRelativeTime(model.lastModified)}</span>
          <button
            aria-label={`Select ${model.displayName}`}
            className="local-row-menu"
            onClick={(event) => {
              event.stopPropagation();
              onSelect(model.id);
            }}
            type="button"
          >
            ...
          </button>
        </div>
      ))}
      {models.length === 0 ? (
        <div className="local-model-empty-row" role="row">
          No models match the current search and filters.
        </div>
      ) : null}
    </div>
  );
}

function SortHeader({
  activeSortKey,
  direction,
  label,
  onSort,
  sortKey,
}: {
  activeSortKey: LocalSortKey;
  direction: SortDirection;
  label: string;
  onSort: (sortKey: LocalSortKey) => void;
  sortKey: LocalSortKey;
}) {
  const isActive = activeSortKey === sortKey;

  return (
    <button className="local-column-sort" data-active={isActive} onClick={() => onSort(sortKey)} role="columnheader" type="button">
      {label}
      {isActive ? <span>{direction === "asc" ? "up" : "down"}</span> : null}
    </button>
  );
}

function LocalModelDetailPanel({
  deleting,
  model,
  onCopyId,
  onCopyPath,
  onDelete,
  onOpenPath,
}: {
  deleting: boolean;
  model: LocalModel | null;
  onCopyId: (model: LocalModel) => void;
  onCopyPath: (model: LocalModel) => void;
  onDelete: (model: LocalModel) => void;
  onOpenPath: (model: LocalModel) => void;
}) {
  if (!model) {
    return (
      <aside className="local-detail-panel">
        <h3>Select a model</h3>
        <p>Choose a local model to inspect metadata and available actions.</p>
      </aside>
    );
  }

  const canDelete = Boolean(model.path && model.source !== "ollama");
  const capabilities = capabilityLabels(model);

  return (
    <aside className="local-detail-panel">
      <div className="local-detail-heading">
        <span className="local-detail-icon" aria-hidden="true">
          {sourceShortLabels[model.source]}
        </span>
        <div>
          <h3>{model.displayName}</h3>
          <p>{model.repoId ?? model.provider ?? model.id}</p>
        </div>
      </div>

      <div className="local-detail-chip-row">
        <span>{formatModelFormat(model.format)}</span>
        {model.quantization ? <span>{model.quantization}</span> : null}
        {model.technical.architecture ? <span>{model.technical.architecture}</span> : null}
      </div>

      <dl className="local-detail-list">
        <DetailItem label="Source" value={sourceLabels[model.source]} />
        <DetailItem label="File Size" value={formatBytes(model.sizeBytes, "Unknown")} />
        <DetailItem label="Files" value={String(model.files.length || 1)} />
        <DetailItem label="Format" value={formatModelFormat(model.format)} />
        <DetailItem label="Quantization" value={model.quantization ?? "Not available"} />
        <DetailItem label="Architecture" value={model.technical.architecture ?? "Not available"} />
        <DetailItem label="Parameters" value={model.parameterSize ?? model.technical.parameterSize ?? formatParameterCount(model.technical.parameterCount)} />
        <DetailItem label="Context Length" value={formatContextLength(model.technical.maxContextLength ?? model.technical.contextLength)} />
        <DetailItem label="License" value={model.provenance.license ?? "Not available"} />
        <DetailItem label="Modified" value={formatScanTimestamp(model.lastModified)} />
      </dl>

      <div className="local-detail-section">
        <span className="field-label">Capabilities</span>
        <div className="local-detail-chip-row">
          {capabilities.length > 0 ? capabilities.map((label) => <span key={label}>{label}</span>) : <span>Not available</span>}
        </div>
      </div>

      <div className="local-detail-section">
        <span className="field-label">Metadata Sources</span>
        <div className="local-detail-chip-row">
          {model.metadataSources.length > 0
            ? model.metadataSources.map((source) => <span key={source}>{formatMetadataSource(source)}</span>)
            : <span>Filesystem</span>}
        </div>
      </div>

      <div className="local-detail-section">
        <span className="field-label">Location</span>
        <code className="local-detail-path">{model.path ?? "Managed by runtime"}</code>
      </div>

      <div className="local-detail-actions">
        <button className="primary-button" disabled={!model.path} onClick={() => onOpenPath(model)} type="button">
          Open in Folder
        </button>
        <button className="secondary-button" onClick={() => onCopyId(model)} type="button">
          Copy Model ID
        </button>
        <button className="secondary-button" disabled={!model.path} onClick={() => onCopyPath(model)} type="button">
          Copy Path
        </button>
        <button className="danger-ghost-button" disabled={!canDelete || deleting} onClick={() => onDelete(model)} type="button">
          {deleting ? "Deleting..." : "Delete Model"}
        </button>
      </div>
    </aside>
  );
}

function DetailItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function Pagination({ currentPage, pageCount, onPageChange }: { currentPage: number; pageCount: number; onPageChange: (page: number) => void }) {
  const pages = Array.from({ length: pageCount }, (_value, index) => index + 1).slice(0, 5);

  return (
    <nav className="local-pagination" aria-label="Local model pages">
      <button disabled={currentPage === 1} onClick={() => onPageChange(currentPage - 1)} type="button">
        Previous
      </button>
      {pages.map((pageNumber) => (
        <button data-active={pageNumber === currentPage} key={pageNumber} onClick={() => onPageChange(pageNumber)} type="button">
          {pageNumber}
        </button>
      ))}
      <button disabled={currentPage === pageCount} onClick={() => onPageChange(currentPage + 1)} type="button">
        Next
      </button>
    </nav>
  );
}

function StatusBanner({ label, message, tone }: { label: string; message: string; tone: "info" | "warning" | "error" }) {
  return (
    <section aria-atomic="true" aria-live="polite" className="status-banner" data-tone={tone}>
      <strong>{label}</strong>
      <span>{message}</span>
    </section>
  );
}

function SourceStatusNotice({ status }: { status: SourceStatus }) {
  const sourceLabel = sourceLabels[status.source];
  const tone = sourceStatusTone(status);
  const message = status.message ?? sourceStatusFallback(status);

  return <StatusBanner label={sourceLabel} message={status.path ? `${message} ${status.path}` : message} tone={tone} />;
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

function countSources(models: LocalModel[]): Record<ModelSource, number> {
  return sourceOrder.reduce(
    (counts, source) => ({ ...counts, [source]: models.filter((model) => model.source === source).length }),
    { huggingface: 0, lmstudio: 0, ollama: 0, custom: 0 },
  );
}

function activeSourceCount(counts: Record<ModelSource, number>): number {
  return sourceOrder.filter((source) => counts[source] > 0).length;
}

function filterModels(models: LocalModel[], sourceFilter: LocalSourceFilter, query: string): LocalModel[] {
  const normalizedQuery = query.trim().toLowerCase();

  return models.filter((model) => {
    if (sourceFilter !== "all" && model.source !== sourceFilter) {
      return false;
    }

    if (!normalizedQuery) {
      return true;
    }

    return searchableModelText(model).includes(normalizedQuery);
  });
}

function searchableModelText(model: LocalModel): string {
  return [
    model.displayName,
    model.repoId,
    model.provider,
    model.id,
    model.quantization,
    model.parameterSize,
    model.technical.architecture,
    model.technical.family,
    ...model.technical.families,
    ...model.provenance.tags,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

function sortModels(models: LocalModel[], sortKey: LocalSortKey, direction: SortDirection): LocalModel[] {
  const multiplier = direction === "asc" ? 1 : -1;

  return [...models].sort((left, right) => compareModels(left, right, sortKey) * multiplier);
}

function compareModels(left: LocalModel, right: LocalModel, sortKey: LocalSortKey): number {
  if (sortKey === "files") {
    return left.files.length - right.files.length;
  }

  if (sortKey === "size") {
    return (left.sizeBytes ?? 0) - (right.sizeBytes ?? 0);
  }

  if (sortKey === "modified") {
    return timestampValue(left.lastModified) - timestampValue(right.lastModified);
  }

  const leftValue = sortValue(left, sortKey);
  const rightValue = sortValue(right, sortKey);

  return leftValue.localeCompare(rightValue);
}

function sortValue(model: LocalModel, sortKey: LocalSortKey): string {
  if (sortKey === "source") {
    return sourceLabels[model.source];
  }

  if (sortKey === "format") {
    return formatModelFormat(model.format);
  }

  if (sortKey === "quantization") {
    return model.quantization ?? "";
  }

  return model.displayName;
}

function timestampValue(value: string | null): number {
  if (!value || value === "0") {
    return 0;
  }

  const numericValue = Number(value);
  const date = Number.isFinite(numericValue) ? new Date(numericValue * 1000) : new Date(value);

  return Number.isNaN(date.getTime()) ? 0 : date.getTime();
}

function paginationLabel(total: number, currentPage: number): string {
  if (total === 0) {
    return "Showing 0 models";
  }

  const start = (currentPage - 1) * pageSize + 1;
  const end = Math.min(currentPage * pageSize, total);

  return `Showing ${start} to ${end} of ${total} models`;
}

function formatModelFormat(format: ModelFormat | null): string {
  return format ? formatLabels[format] : "Unknown";
}

function formatParameterCount(value: number | null): string {
  if (value === null) {
    return "Not available";
  }

  if (value >= 1_000_000_000) {
    return `${formatCompactNumber(value / 1_000_000_000)}B`;
  }

  if (value >= 1_000_000) {
    return `${formatCompactNumber(value / 1_000_000)}M`;
  }

  return new Intl.NumberFormat().format(value);
}

function formatContextLength(value: number | null): string {
  if (value === null) {
    return "Not available";
  }

  if (value >= 1024 && value % 1024 === 0) {
    return `${value / 1024}K`;
  }

  return new Intl.NumberFormat().format(value);
}

function formatCompactNumber(value: number): string {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: value >= 10 ? 0 : 1 }).format(value);
}

function capabilityLabels(model: LocalModel): string[] {
  const labels: string[] = [];

  if (model.capabilities.vision) {
    labels.push("Vision");
  }

  if (model.capabilities.embedding) {
    labels.push("Embedding");
  }

  if (model.capabilities.toolUse) {
    labels.push("Tool use");
  }

  if (model.capabilities.reasoning) {
    labels.push("Reasoning");
  }

  return labels;
}

function formatMetadataSource(source: string): string {
  return source
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
