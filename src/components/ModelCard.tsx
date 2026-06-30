import type { LocalModel, ModelFormat, ModelSource } from "../api/types";

type ModelCardProps = {
  deleting: boolean;
  model: LocalModel;
  onCopyId: (model: LocalModel) => void;
  onCopyPath: (model: LocalModel) => void;
  onDelete: (model: LocalModel) => void;
  onOpenPath: (model: LocalModel) => void;
};

const sourceLabels: Record<ModelSource, string> = {
  huggingface: "Hugging Face",
  lmstudio: "LM Studio",
  ollama: "Ollama",
  custom: "Custom",
};

const formatLabels: Record<ModelFormat, string> = {
  gguf: "GGUF",
  safetensors: "Safetensors",
  onnx: "ONNX",
  mlx: "MLX",
  unknown: "Unknown format",
};

export function ModelCard({
  deleting,
  model,
  onCopyId,
  onCopyPath,
  onDelete,
  onOpenPath,
}: ModelCardProps) {
  const identity = model.repoId ?? model.provider ?? model.id;
  const fileCount = model.files.length;
  const hasPath = model.path !== null;
  const canDelete = hasPath && model.source !== "ollama";

  return (
    <article className="model-card">
      <div className="model-card-heading">
        <div>
          <span className="source-chip" data-source={model.source}>
            {sourceLabels[model.source]}
          </span>
          <h4>{model.displayName}</h4>
        </div>
        <span className="model-size">{formatBytes(model.sizeBytes)}</span>
      </div>

      <p className="model-identity">{identity}</p>

      <dl className="model-meta-grid">
        <MetaItem label="Format" value={formatModelFormat(model.format)} />
        <MetaItem label="Quant" value={model.quantization ?? "Unknown"} />
        <MetaItem label="Params" value={model.parameterSize ?? "Unknown"} />
        <MetaItem label="Files" value={`${fileCount} ${fileCount === 1 ? "file" : "files"}`} />
        <MetaItem label="Runtime" value={formatRuntimeStatus(model.runtimeStatus)} />
        <MetaItem label="Modified" value={formatScanTimestamp(model.lastModified)} />
      </dl>

      {model.path ? <code className="model-path">{model.path}</code> : null}

      <div className="model-action-row" aria-label={`${model.displayName} actions`}>
        <button className="secondary-button" onClick={() => onCopyId(model)} type="button">
          Copy ID
        </button>
        {hasPath ? (
          <>
            <button className="secondary-button" onClick={() => onOpenPath(model)} type="button">
              Open folder
            </button>
            <button className="secondary-button" onClick={() => onCopyPath(model)} type="button">
              Copy path
            </button>
          </>
        ) : null}
        {canDelete ? (
          <button
            className="danger-ghost-button"
            disabled={deleting}
            onClick={() => onDelete(model)}
            type="button"
          >
            {deleting ? "Deleting..." : "Delete"}
          </button>
        ) : null}
      </div>
    </article>
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

function formatModelFormat(format: ModelFormat | null): string {
  return format ? formatLabels[format] : "Unknown format";
}

function formatRuntimeStatus(status: LocalModel["runtimeStatus"]): string {
  if (!status) {
    return "Unknown";
  }

  return status.charAt(0).toUpperCase() + status.slice(1);
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) {
    return "Size unavailable";
  }

  if (bytes === 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** unitIndex;
  const precision = value >= 10 || unitIndex === 0 ? 0 : 1;

  return `${value.toFixed(precision)} ${units[unitIndex]}`;
}

export function formatScanTimestamp(value: string | null): string {
  if (!value || value === "0") {
    return "time unavailable";
  }

  const numericValue = Number(value);
  const date = Number.isFinite(numericValue)
    ? new Date(numericValue * 1000)
    : new Date(value);

  if (Number.isNaN(date.getTime())) {
    return "time unavailable";
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
