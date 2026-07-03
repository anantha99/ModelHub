import type { LocalModel, ModelFormat, ModelSource } from "../api/types";
import { formatBytes, formatScanTimestamp } from "../utils/format";

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
  const metadataItems = [
    isKnownFormat(model.format)
      ? { label: "Format", value: formatModelFormat(model.format) }
      : null,
    hasUsefulMetadataValue(model.quantization)
      ? { label: "Quant", value: model.quantization }
      : null,
    hasUsefulMetadataValue(model.parameterSize)
      ? { label: "Params", value: model.parameterSize }
      : null,
    { label: "Files", value: `${fileCount} ${fileCount === 1 ? "file" : "files"}` },
    hasKnownRuntimeStatus(model.runtimeStatus)
      ? { label: "Runtime", value: formatRuntimeStatus(model.runtimeStatus) }
      : null,
    hasUsefulTimestamp(model.lastModified)
      ? { label: "Modified", value: formatScanTimestamp(model.lastModified) }
      : null,
  ].filter((item): item is { label: string; value: string } => item !== null);

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
        {metadataItems.map((item) => (
          <MetaItem key={item.label} label={item.label} value={item.value} />
        ))}
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
            {deleting ? "Deleting…" : "Delete"}
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

function isKnownFormat(format: ModelFormat | null): boolean {
  return format !== null && format !== "unknown";
}

function hasUsefulMetadataValue(value: string | null): value is string {
  const normalizedValue = value?.trim().toLowerCase();

  return Boolean(normalizedValue && normalizedValue !== "unknown");
}

function hasUsefulTimestamp(value: string | null): value is string {
  return Boolean(value && value !== "0");
}

function hasKnownRuntimeStatus(status: LocalModel["runtimeStatus"]): boolean {
  return Boolean(status && status !== "unknown");
}

function formatRuntimeStatus(status: LocalModel["runtimeStatus"]): string {
  if (!status) {
    return "Unknown";
  }

  return status.charAt(0).toUpperCase() + status.slice(1);
}
