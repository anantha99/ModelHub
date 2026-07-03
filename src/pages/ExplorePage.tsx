import { useEffect, useMemo, useRef, useState } from "react";
import { getHfModelDetails, searchHfModels, startDownload } from "../api/tauri";
import type {
  HfModelDetails,
  HfModelFile,
  HfModelSummary,
  HfSearchFilters,
  HfSearchInput,
  HfSearchResult,
  HfSearchSort,
} from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { formatBytes, formatCount, formatScanTimestamp } from "../utils/format";

const defaultFilters: HfSearchFilters = {
  textGeneration: true,
  gguf: false,
  safetensors: false,
};

const sortLabels: Record<HfSearchSort, string> = {
  downloads: "Downloads",
  likes: "Likes",
  last_modified: "Recently updated",
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatTag(tag: string): string {
  return tag.replace(/^license:/, "");
}

function fileSummaryLabel(model: HfModelSummary): string {
  const parts = [];

  if (model.fileSummary.ggufFiles > 0) {
    parts.push(`${model.fileSummary.ggufFiles} GGUF`);
  }

  if (model.fileSummary.safetensorsFiles > 0) {
    parts.push(`${model.fileSummary.safetensorsFiles} Safetensors`);
  }

  if (model.fileSummary.configFiles > 0) {
    parts.push(`${model.fileSummary.configFiles} config`);
  }

  if (model.fileSummary.tokenizerFiles > 0) {
    parts.push(`${model.fileSummary.tokenizerFiles} tokenizer`);
  }

  if (parts.length > 0) {
    return parts.join(" / ");
  }

  if (model.fileSummary.totalFiles > 0) {
    return `${model.fileSummary.totalFiles} files`;
  }

  return "File summary unavailable";
}

function searchInput(query: string, filters: HfSearchFilters, sort: HfSearchSort): HfSearchInput {
  return {
    query,
    filters,
    sort,
    limit: 25,
  };
}

export function ExplorePage() {
  const [query, setQuery] = useState("");
  const [lastSubmittedInput, setLastSubmittedInput] = useState<HfSearchInput | null>(null);
  const [filters, setFilters] = useState<HfSearchFilters>(defaultFilters);
  const [sort, setSort] = useState<HfSearchSort>("downloads");
  const [result, setResult] = useState<HfSearchResult | null>(null);
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);
  const [details, setDetails] = useState<HfModelDetails | null>(null);
  const [selectedFilePaths, setSelectedFilePaths] = useState<Set<string>>(new Set());
  const [isLoadingDetails, setIsLoadingDetails] = useState(false);
  const [detailsError, setDetailsError] = useState<string | null>(null);
  const [downloadMessage, setDownloadMessage] = useState<string | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [isStartingDownload, setIsStartingDownload] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);
  const latestDetailsRequestId = useRef(0);

  const selectedModel = useMemo(
    () => result?.models.find((model) => model.repoId === selectedRepoId) ?? null,
    [result, selectedRepoId],
  );

  const selectedFiles = useMemo(
    () => details?.files.filter((file) => selectedFilePaths.has(file.path)) ?? [],
    [details, selectedFilePaths],
  );

  const selectedBytes = useMemo(() => sumKnownBytes(selectedFiles), [selectedFiles]);

  useEffect(() => {
    if (!selectedRepoId) {
      setDetails(null);
      setDetailsError(null);
      setSelectedFilePaths(new Set());
      return;
    }

    const requestId = latestDetailsRequestId.current + 1;
    latestDetailsRequestId.current = requestId;
    setIsLoadingDetails(true);
    setDetailsError(null);
    setDownloadMessage(null);
    setDownloadError(null);

    void getHfModelDetails(selectedRepoId)
      .then((modelDetails) => {
        if (requestId !== latestDetailsRequestId.current) {
          return;
        }

        setDetails(modelDetails);
        setSelectedFilePaths(new Set(modelDetails.files.filter((file) => file.likelyDefault).map((file) => file.path)));
      })
      .catch((detailsLoadError) => {
        if (requestId !== latestDetailsRequestId.current) {
          return;
        }

        setDetails(null);
        setSelectedFilePaths(new Set());
        setDetailsError(errorMessage(detailsLoadError));
      })
      .finally(() => {
        if (requestId === latestDetailsRequestId.current) {
          setIsLoadingDetails(false);
        }
      });
  }, [selectedRepoId]);

  async function runSearch(input: HfSearchInput) {
    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;

    setIsSearching(true);
    setError(null);

    try {
      const searchResult = await searchHfModels(input);

      if (requestId !== latestRequestId.current) {
        return;
      }

      setResult(searchResult);
      setLastSubmittedInput(input);
      setSelectedRepoId(searchResult.models[0]?.repoId ?? null);
    } catch (searchError) {
      if (requestId !== latestRequestId.current) {
        return;
      }

      setError(errorMessage(searchError));
    } finally {
      if (requestId === latestRequestId.current) {
        setIsSearching(false);
      }
    }
  }

  function submitSearch() {
    const input = searchInput(query, filters, sort);
    void runSearch(input);
  }

  function retrySearch() {
    if (lastSubmittedInput) {
      void runSearch(lastSubmittedInput);
    } else {
      submitSearch();
    }
  }

  function updateFilter(filter: keyof HfSearchFilters, value: boolean) {
    setFilters((current) => ({ ...current, [filter]: value }));
  }

  function updateFileSelection(filePath: string, selected: boolean) {
    setSelectedFilePaths((current) => {
      const next = new Set(current);

      if (selected) {
        next.add(filePath);
      } else {
        next.delete(filePath);
      }

      return next;
    });
  }

  async function startSelectedDownload() {
    if (!details || selectedFiles.length === 0) {
      return;
    }

    setIsStartingDownload(true);
    setDownloadError(null);
    setDownloadMessage(null);

    try {
      const job = await startDownload({
        repoId: details.repoId,
        revision: details.revision,
        commitSha: details.commitSha,
        files: selectedFiles,
        destination: "staging",
      });

      setDownloadMessage(`Started ${job.files.length} file download for ${job.repoId}. Track it on Downloads.`);
    } catch (downloadStartError) {
      setDownloadError(errorMessage(downloadStartError));
    } finally {
      setIsStartingDownload(false);
    }
  }

  return (
    <div className="page-stack explore-page">
      <header className="page-header explore-hero">
        <div>
          <p className="eyebrow">Explore</p>
          <h2>Hugging Face search</h2>
          <p>
            Search public models, compare formats, and inspect promising repos before
            downloading files in the next workflow slice.
          </p>
        </div>
      </header>

      <section className="settings-card explore-search-card" aria-label="Hugging Face search form">
        <div className="explore-search-row">
          <label className="field-stack explore-query-field" htmlFor="hf-search-query">
            <span className="field-label">Model search</span>
            <input
              autoComplete="off"
              className="text-input"
              id="hf-search-query"
              name="hfSearchQuery"
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  submitSearch();
                }
              }}
              placeholder="e.g. qwen, mistral, llama gguf…"
              value={query}
            />
          </label>

          <label className="field-stack explore-sort-field" htmlFor="hf-search-sort">
            <span className="field-label">Sort by</span>
            <select
              autoComplete="off"
              className="text-input"
              id="hf-search-sort"
              name="hfSearchSort"
              onChange={(event) => setSort(event.target.value as HfSearchSort)}
              value={sort}
            >
              {Object.entries(sortLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          <button
            className="primary-button explore-search-button"
            disabled={isSearching}
            onClick={submitSearch}
            type="button"
          >
            {isSearching ? "Searching…" : "Search"}
          </button>
        </div>

        <div className="explore-filter-row" aria-label="Search filters">
          <FilterToggle
            checked={filters.textGeneration}
            label="Text generation"
            name="textGenerationFilter"
            onChange={(checked) => updateFilter("textGeneration", checked)}
          />
          <FilterToggle
            checked={filters.gguf}
            label="GGUF"
            name="ggufFilter"
            onChange={(checked) => updateFilter("gguf", checked)}
          />
          <FilterToggle
            checked={filters.safetensors}
            label="Safetensors"
            name="safetensorsFilter"
            onChange={(checked) => updateFilter("safetensors", checked)}
          />
        </div>
      </section>

      {error ? (
        <section aria-atomic="true" aria-live="polite" className="status-banner" data-tone="error">
          <strong>Search failed</strong>
          <span>{error}</span>
          <button className="secondary-button" disabled={isSearching} onClick={retrySearch} type="button">
            Retry
          </button>
        </section>
      ) : null}

      {isSearching && !result ? (
        <section className="settings-card explore-loading-card">
          <p className="eyebrow">Searching</p>
          <h3>Checking Hugging Face models</h3>
          <p>ModelHub is querying the public Hub API without sending local paths.</p>
        </section>
      ) : result && result.models.length > 0 ? (
        <div className="explore-results-layout">
          <section className="explore-results-list" aria-label="Hugging Face search results">
            <div className="section-heading-row">
              <div>
                <p className="eyebrow">Results</p>
                <h3>
                  {`${result.models.length} repos for “${result.query}”`}
                </h3>
              </div>
              <span className="soft-pill">{sortLabels[sort]}</span>
            </div>

            <div className="hf-result-grid">
              {result.models.map((model) => (
                <HfResultCard
                  isSelected={model.repoId === selectedRepoId}
                  key={model.repoId}
                  model={model}
                  onSelect={() => setSelectedRepoId(model.repoId)}
                />
              ))}
            </div>
          </section>

          <HfSelectedModelPanel
            details={details}
            detailsError={detailsError}
            downloadError={downloadError}
            downloadMessage={downloadMessage}
            isLoadingDetails={isLoadingDetails}
            isStartingDownload={isStartingDownload}
            model={selectedModel}
            onFileSelectionChange={updateFileSelection}
            onStartDownload={startSelectedDownload}
            selectedBytes={selectedBytes}
            selectedFilePaths={selectedFilePaths}
            selectedFiles={selectedFiles}
          />
        </div>
      ) : result ? (
        <EmptyState
          eyebrow="No results"
          title={`No Hugging Face models found for “${result.query}”`}
          description="Try a broader keyword or remove format filters. Public search works without a Hugging Face token."
        />
      ) : (
        <EmptyState
          eyebrow="Public Hub search"
          title="Find a model to download later"
          description="Search by repo name, model family, quantization, or format. Results stay in the app and local paths are never sent to Hugging Face."
        />
      )}
    </div>
  );
}

function FilterToggle({
  checked,
  label,
  name,
  onChange,
}: {
  checked: boolean;
  label: string;
  name: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="explore-filter-toggle">
      <input
        autoComplete="off"
        checked={checked}
        name={name}
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
      <span>{label}</span>
    </label>
  );
}

function HfResultCard({
  isSelected,
  model,
  onSelect,
}: {
  isSelected: boolean;
  model: HfModelSummary;
  onSelect: () => void;
}) {
  const visibleTags = model.tags.slice(0, 5);

  return (
    <button
      aria-pressed={isSelected}
      className="hf-result-card"
      data-selected={isSelected}
      onClick={onSelect}
      type="button"
    >
      <span className="hf-result-card-topline">
        <span className="source-chip" data-source="huggingface">
          Hugging Face
        </span>
        {model.gated ? <span className="warning-pill">Gated</span> : null}
        {model.private ? <span className="warning-pill">Private</span> : null}
      </span>

      <span className="hf-result-title">{model.repoId}</span>
      <span className="model-identity">{model.author ?? "Unknown author"}</span>

      <dl className="hf-result-meta-grid">
        <MetaItem label="Downloads" value={formatCount(model.downloads)} />
        <MetaItem label="Likes" value={formatCount(model.likes)} />
        <MetaItem label="Updated" value={formatScanTimestamp(model.lastModified)} />
      </dl>

      <span className="hf-file-summary">{fileSummaryLabel(model)}</span>

      {visibleTags.length > 0 ? (
        <span className="hf-tag-list">
          {visibleTags.map((tag) => (
            <span className="hf-tag" key={tag}>
              {formatTag(tag)}
            </span>
          ))}
        </span>
      ) : null}
    </button>
  );
}

function HfSelectedModelPanel({
  details,
  detailsError,
  downloadError,
  downloadMessage,
  isLoadingDetails,
  isStartingDownload,
  model,
  onFileSelectionChange,
  onStartDownload,
  selectedBytes,
  selectedFilePaths,
  selectedFiles,
}: {
  details: HfModelDetails | null;
  detailsError: string | null;
  downloadError: string | null;
  downloadMessage: string | null;
  isLoadingDetails: boolean;
  isStartingDownload: boolean;
  model: HfModelSummary | null;
  onFileSelectionChange: (filePath: string, selected: boolean) => void;
  onStartDownload: () => void;
  selectedBytes: number | null;
  selectedFilePaths: Set<string>;
  selectedFiles: HfModelFile[];
}) {
  if (!model) {
    return (
      <aside className="settings-card explore-detail-panel">
        <p className="eyebrow">Model details</p>
        <h3>Select a result</h3>
        <p>Choose a Hugging Face result to inspect repo metadata and file signals.</p>
      </aside>
    );
  }

  return (
    <aside className="settings-card explore-detail-panel">
      <p className="eyebrow">Selected model</p>
      <h3>{model.repoId}</h3>
      <p>{model.pipelineTag ?? "Pipeline unavailable"}</p>

      <dl className="model-meta-grid">
        <MetaItem label="Author" value={model.author ?? "Unknown"} />
        <MetaItem label="Downloads" value={formatCount(model.downloads)} />
        <MetaItem label="Likes" value={formatCount(model.likes)} />
        <MetaItem label="Updated" value={formatScanTimestamp(model.lastModified)} />
        <MetaItem label="Access" value={model.gated ? "Gated" : model.private ? "Private" : "Public"} />
        <MetaItem label="Files" value={fileSummaryLabel(model)} />
      </dl>

      <div className="hf-detail-section">
        <span className="field-label">Tags</span>
        {model.tags.length > 0 ? (
          <div className="hf-tag-list">
            {model.tags.slice(0, 12).map((tag) => (
              <span className="hf-tag" key={tag}>
                {formatTag(tag)}
              </span>
            ))}
          </div>
        ) : (
          <p>No tags returned.</p>
        )}
      </div>

      {detailsError ? (
        <div aria-atomic="true" aria-live="polite" className="status-banner" data-tone="error">
          <strong>Details failed</strong>
          <span>{detailsError}</span>
        </div>
      ) : null}

      {downloadMessage ? (
        <div aria-atomic="true" aria-live="polite" className="status-banner" data-tone="success">
          <strong>Download started</strong>
          <span>{downloadMessage}</span>
        </div>
      ) : null}

      {downloadError ? (
        <div aria-atomic="true" aria-live="polite" className="status-banner" data-tone="error">
          <strong>Download failed</strong>
          <span>{downloadError}</span>
        </div>
      ) : null}

      {isLoadingDetails ? (
        <div className="hf-file-loading">
          <p className="eyebrow">Loading files</p>
          <p>Fetching repo file metadata from Hugging Face.</p>
        </div>
      ) : details ? (
        <div className="hf-file-picker">
          <div className="section-heading-row">
            <div>
              <p className="eyebrow">Files</p>
              <h4>Select files to stage</h4>
            </div>
            <span className="soft-pill">{details.files.length} files</span>
          </div>

          <div className="download-selection-summary">
            <strong>{selectedFiles.length} selected</strong>
            <span>{formatBytes(selectedBytes, "Unknown size")}</span>
          </div>

          {selectedBytes !== null && selectedBytes > 5 * 1024 * 1024 * 1024 ? (
            <div aria-atomic="true" aria-live="polite" className="status-banner" data-tone="warning">
              <strong>Large download</strong>
              <span>Selected files exceed 5&nbsp;GB. Make sure you have disk space before continuing.</span>
            </div>
          ) : null}

          <div className="hf-file-groups">
            {fileGroups(details.files).map((group) => (
              <div className="hf-file-group" key={group.label}>
                <h5>{group.label}</h5>
                {group.files.map((file) => (
                  <label className="hf-file-row" key={file.path}>
                    <input
                      autoComplete="off"
                      checked={selectedFilePaths.has(file.path)}
                      name="hfFileSelection"
                      onChange={(event) => onFileSelectionChange(file.path, event.target.checked)}
                      type="checkbox"
                    />
                    <span>
                      <strong>{file.path}</strong>
                      <small>
                        {formatBytes(file.sizeBytes, "Unknown size")} / {file.format}
                        {file.lfs ? " / LFS" : ""}
                      </small>
                    </span>
                  </label>
                ))}
              </div>
            ))}
          </div>

          <button
            className="primary-button"
            disabled={selectedFiles.length === 0 || isStartingDownload}
            onClick={onStartDownload}
            type="button"
          >
            {isStartingDownload ? "Starting…" : "Start download"}
          </button>
        </div>
      ) : null}
    </aside>
  );
}

function MetaItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function fileGroups(files: HfModelFile[]): Array<{ label: string; files: HfModelFile[] }> {
  const groups = [
    { label: "GGUF", files: files.filter((file) => file.format === "gguf") },
    { label: "Safetensors", files: files.filter((file) => file.format === "safetensors") },
    {
      label: "Tokenizer / config",
      files: files.filter((file) => file.format === "unknown" && isTokenizerOrConfig(file.path)),
    },
    {
      label: "Other",
      files: files.filter(
        (file) => file.format !== "gguf" && file.format !== "safetensors" && !isTokenizerOrConfig(file.path),
      ),
    },
  ];

  return groups.filter((group) => group.files.length > 0);
}

function isTokenizerOrConfig(path: string): boolean {
  const lowerPath = path.toLowerCase();

  return lowerPath.endsWith("config.json") || lowerPath.includes("tokenizer");
}

function sumKnownBytes(files: HfModelFile[]): number | null {
  let total = 0;

  for (const file of files) {
    if (file.sizeBytes === null) {
      return null;
    }

    total += file.sizeBytes;
  }

  return total;
}
