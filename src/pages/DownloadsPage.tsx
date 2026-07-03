import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { cancelDownload, installDownload, listDownloads } from "../api/tauri";
import type { DownloadJob, DownloadStatus } from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { formatBytes } from "../utils/format";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function formatDownloadBytes(value: number | null): string {
  return formatBytes(value, "Unknown size");
}

function formatJobTime(value: string): string {
  const millis = Number(value);

  if (!Number.isFinite(millis) || millis <= 0) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(millis));
}

function statusLabel(status: DownloadStatus): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function progressPercent(job: DownloadJob): number | null {
  if (job.totalBytes === null || job.totalBytes <= 0) {
    return null;
  }

  return Math.min(100, Math.round((job.downloadedBytes / job.totalBytes) * 100));
}

function progressValueText(job: DownloadJob, percent: number | null): string {
  if (percent === null) {
    return `Downloaded ${formatDownloadBytes(job.downloadedBytes)} of an unknown total`;
  }

  return `Downloaded ${formatDownloadBytes(job.downloadedBytes)} of ${formatDownloadBytes(job.totalBytes)} (${percent}%)`;
}

export function DownloadsPage() {
  const [jobs, setJobs] = useState<DownloadJob[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [cancelError, setCancelError] = useState<string | null>(null);
  const [cancellingJobId, setCancellingJobId] = useState<string | null>(null);
  const [installingJobId, setInstallingJobId] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);

  const sortedJobs = useMemo(
    () => [...jobs].sort((left, right) => Number(right.updatedAt) - Number(left.updatedAt)),
    [jobs],
  );

  useEffect(() => {
    let mounted = true;

    void listDownloads()
      .then((downloadJobs) => {
        if (!mounted) {
          return;
        }

        setJobs(downloadJobs);
        setError(null);
      })
      .catch((loadError) => {
        if (!mounted) {
          return;
        }

        setError(errorMessage(loadError));
      })
      .finally(() => {
        if (mounted) {
          setIsLoading(false);
        }
      });

    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    const unlisteners = [
      listen<DownloadJob>("download:updated", (event) => upsertJob(event.payload)),
      listen<DownloadJob>("download:completed", (event) => upsertJob(event.payload)),
      listen<DownloadJob>("download:failed", (event) => upsertJob(event.payload)),
    ];

    return () => {
      for (const unlisten of unlisteners) {
        void unlisten.then((dispose) => dispose());
      }
    };
  }, []);

  function upsertJob(job: DownloadJob) {
    setJobs((current) => {
      const existingIndex = current.findIndex((candidate) => candidate.id === job.id);

      if (existingIndex === -1) {
        return [job, ...current];
      }

      const next = [...current];
      next[existingIndex] = job;
      return next;
    });
  }

  async function cancelJob(job: DownloadJob) {
    const confirmed = window.confirm(
      `Cancel download for “${job.repoId}”? Partial files may be discarded.`,
    );

    if (!confirmed) {
      return;
    }

    setCancellingJobId(job.id);
    setCancelError(null);

    try {
      await cancelDownload(job.id);
    } catch (cancelDownloadError) {
      setCancelError(errorMessage(cancelDownloadError));
    } finally {
      setCancellingJobId(null);
    }
  }

  async function installJob(jobId: string) {
    setInstallingJobId(jobId);
    setInstallError(null);

    try {
      const result = await installDownload(jobId);
      setJobs((current) =>
        current.map((job) =>
          job.id === jobId
            ? {
                ...job,
                installedAt: Date.now().toString(),
                cachePath: result.cachePath,
                snapshotPath: result.snapshotPath,
                installError: null,
                installWarnings: result.warnings,
              }
            : job,
        ),
      );
    } catch (installDownloadError) {
      setInstallError(errorMessage(installDownloadError));
    } finally {
      setInstallingJobId(null);
    }
  }

  return (
    <div className="page-stack downloads-page">
      <header className="page-header">
        <p className="eyebrow">Downloads</p>
        <h2>Download manager</h2>
        <p>Track staged Hugging Face downloads, live progress, cancellations, and failed jobs.</p>
      </header>

      {error ? (
        <section aria-atomic="true" aria-live="polite" className="status-banner" data-tone="error">
          <strong>Downloads failed</strong>
          <span>{error}</span>
        </section>
      ) : null}

      {cancelError ? (
        <section aria-atomic="true" aria-live="polite" className="status-banner" data-tone="error">
          <strong>Cancel failed</strong>
          <span>{cancelError}</span>
        </section>
      ) : null}

      {installError ? (
        <section aria-atomic="true" aria-live="polite" className="status-banner" data-tone="error">
          <strong>Install failed</strong>
          <span>{installError}</span>
        </section>
      ) : null}

      {isLoading ? (
        <section className="settings-card downloads-loading-card">
          <p className="eyebrow">Loading</p>
          <h3>Reading saved downloads</h3>
          <p>ModelHub is loading persisted download jobs from local app data.</p>
        </section>
      ) : sortedJobs.length > 0 ? (
        <section className="downloads-list" aria-label="Download jobs">
          {sortedJobs.map((job) => (
            <DownloadJobCard
              cancelling={cancellingJobId === job.id}
              installing={installingJobId === job.id}
              job={job}
              key={job.id}
              onCancel={() => void cancelJob(job)}
              onInstall={() => void installJob(job.id)}
            />
          ))}
        </section>
      ) : (
        <EmptyState
          eyebrow="No downloads"
          title="No staged downloads yet"
          description="Search Hugging Face, select files, and start a download. Jobs will appear here with live progress."
        />
      )}
    </div>
  );
}

function DownloadJobCard({
  cancelling,
  installing,
  job,
  onCancel,
  onInstall,
}: {
  cancelling: boolean;
  installing: boolean;
  job: DownloadJob;
  onCancel: () => void;
  onInstall: () => void;
}) {
  const percent = progressPercent(job);
  const canCancel = job.status === "queued" || job.status === "downloading";
  const canInstall = job.status === "completed" && job.installedAt === null;

  return (
    <article className="settings-card download-job-card" data-status={job.status}>
      <div className="download-job-header">
        <div>
          <p className="eyebrow">{statusLabel(job.status)}</p>
          <h3>{job.repoId}</h3>
          <p>
            {job.files.length} files / updated {formatJobTime(job.updatedAt)}
          </p>
        </div>
        <span className="download-status-pill" data-status={job.status}>
          {statusLabel(job.status)}
        </span>
      </div>

      <div className="download-progress-block">
        <div className="download-progress-meta">
          <span>
            {formatDownloadBytes(job.downloadedBytes)} / {formatDownloadBytes(job.totalBytes)}
          </span>
          <span>{percent === null ? "Size unknown" : `${percent}%`}</span>
        </div>
        <div
          aria-label="Download progress"
          aria-valuemax={100}
          aria-valuemin={0}
          aria-valuenow={percent ?? undefined}
          aria-valuetext={progressValueText(job, percent)}
          className="download-progress-track"
          role="progressbar"
        >
          <span style={{ width: `${percent ?? 8}%` }} />
        </div>
      </div>

      {job.error ? <p className="download-error">{job.error}</p> : null}
      {job.installError ? <p className="download-error">{job.installError}</p> : null}

      {job.installedAt ? (
        <div className="download-install-summary">
          <strong>Installed to Hugging Face cache</strong>
          {job.snapshotPath ? <small>Snapshot: {job.snapshotPath}</small> : null}
          {job.cachePath ? <small>Cache: {job.cachePath}</small> : null}
        </div>
      ) : null}

      {job.installWarnings.length > 0 ? (
        <div className="download-warning-list">
          {job.installWarnings.map((warning) => (
            <small key={warning}>{warning}</small>
          ))}
        </div>
      ) : null}

      <div className="download-file-list">
        {job.files.map((file) => (
          <div className="download-file-row" key={file.path}>
            <span>
              <strong>{file.path}</strong>
              <small>
                {formatDownloadBytes(file.downloadedBytes)} / {formatDownloadBytes(file.sizeBytes)}
              </small>
              {file.stagedPath ? <small>{file.stagedPath}</small> : null}
              {file.error ? <small>{file.error}</small> : null}
            </span>
          </div>
        ))}
      </div>

      {canCancel ? (
        <button className="danger-ghost-button" disabled={cancelling} onClick={onCancel} type="button">
          {cancelling ? "Cancelling…" : "Cancel download"}
        </button>
      ) : null}

      {canInstall ? (
        <button className="primary-button" disabled={installing} onClick={onInstall} type="button">
          {installing ? "Installing…" : "Install to HF cache"}
        </button>
      ) : null}
    </article>
  );
}
