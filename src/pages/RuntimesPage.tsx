import { useEffect, useRef, useState } from "react";
import { getOllamaRuntimeStatus } from "../api/tauri";
import type { LocalModel, OllamaRuntimeStatus } from "../api/types";
import { formatBytes, formatScanTimestamp } from "../components/ModelCard";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function RuntimesPage() {
  const [ollamaStatus, setOllamaStatus] = useState<OllamaRuntimeStatus | null>(null);
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isMounted = useRef(false);
  const latestRequestId = useRef(0);

  async function loadOllamaStatus(mode: "initial" | "refresh") {
    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;

    if (mode === "initial") {
      setIsInitialLoading(true);
    } else {
      setIsRefreshing(true);
    }

    setError(null);

    try {
      const status = await getOllamaRuntimeStatus();

      if (!isMounted.current || requestId !== latestRequestId.current) {
        return;
      }

      setOllamaStatus(status);
    } catch (loadError) {
      if (!isMounted.current || requestId !== latestRequestId.current) {
        return;
      }

      setError(errorMessage(loadError));
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
    void loadOllamaStatus("initial");

    return () => {
      isMounted.current = false;
    };
  }, []);

  const canRefresh = !isInitialLoading && !isRefreshing;

  return (
    <div className="page-stack runtimes-page">
      <header className="page-header runtimes-hero">
        <div>
          <p className="eyebrow">Runtimes</p>
          <h2>Ollama status</h2>
          <p>
            Check local runtime availability without requiring Ollama or LM Studio
            to be installed.
          </p>
        </div>
        <button
          className="primary-button"
          disabled={!canRefresh}
          onClick={() => void loadOllamaStatus("refresh")}
          type="button"
        >
          {isRefreshing || isInitialLoading ? "Checking..." : "Refresh"}
        </button>
      </header>

      {error ? (
        <section className="status-banner" data-tone="error">
          <strong>Runtime check failed</strong>
          <span>{error}</span>
          <button
            className="secondary-button"
            disabled={!canRefresh}
            onClick={() => void loadOllamaStatus("refresh")}
            type="button"
          >
            Retry
          </button>
        </section>
      ) : null}

      <div className="runtime-card-grid">
        {isInitialLoading && !ollamaStatus ? (
          <section className="settings-card runtime-card runtime-loading-card">
            <p className="eyebrow">Checking</p>
            <h3>Looking for Ollama</h3>
            <p>ModelHub is checking http://localhost:11434 with a short timeout.</p>
          </section>
        ) : (
          <OllamaRuntimeCard status={ollamaStatus} />
        )}

        <section className="settings-card runtime-card runtime-placeholder-card">
          <div className="section-heading-row">
            <div>
              <p className="eyebrow">LM Studio</p>
              <h3>Runtime check not connected yet</h3>
            </div>
            <span className="soft-pill">Pending</span>
          </div>
          <p>
            LM Studio folder scanning is available on the Local page. Its
            OpenAI-compatible server check will be added with the full runtime
            checks task.
          </p>
          <code className="model-path">http://localhost:1234/v1</code>
        </section>
      </div>
    </div>
  );
}

function OllamaRuntimeCard({ status }: { status: OllamaRuntimeStatus | null }) {
  if (!status) {
    return (
      <section className="settings-card runtime-card">
        <p className="eyebrow">Ollama</p>
        <h3>Status unavailable</h3>
        <p>Refresh to check the local Ollama API.</p>
      </section>
    );
  }

  return (
    <section className="settings-card runtime-card">
      <div className="section-heading-row">
        <div>
          <p className="eyebrow">Ollama</p>
          <h3>{status.running ? "Running" : "Not running"}</h3>
        </div>
        <span className={status.running ? "success-pill" : "warning-pill"}>
          {status.running ? "Online" : "Offline"}
        </span>
      </div>

      <dl className="runtime-summary-grid">
        <RuntimeMeta label="Base URL" value={status.baseUrl} />
        <RuntimeMeta label="Models" value={String(status.models.length)} />
        <RuntimeMeta label="Checked" value={formatScanTimestamp(status.checkedAt)} />
      </dl>

      {status.error ? (
        <section className="status-banner" data-tone={status.running ? "warning" : "info"}>
          <strong>{status.running ? "Warning" : "Not running"}</strong>
          <span>{status.error}</span>
        </section>
      ) : null}

      {status.models.length > 0 ? (
        <div className="runtime-model-list" aria-label="Installed Ollama models">
          {status.models.map((model) => (
            <OllamaModelRow key={model.id} model={model} />
          ))}
        </div>
      ) : status.running ? (
        <p className="field-help">Ollama is running, but it did not return installed models.</p>
      ) : (
        <p className="field-help">Start Ollama, then refresh this page to list installed models.</p>
      )}
    </section>
  );
}

function RuntimeMeta({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function OllamaModelRow({ model }: { model: LocalModel }) {
  return (
    <article className="runtime-model-row">
      <div>
        <strong>{model.displayName}</strong>
        <span>{model.parameterSize ?? "Parameter size unknown"}</span>
      </div>
      <div>
        <span>{model.quantization ?? "Quant unknown"}</span>
        <span>{formatBytes(model.sizeBytes)}</span>
      </div>
    </article>
  );
}
