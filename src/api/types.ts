export type AppPage = "local" | "explore" | "downloads" | "runtimes" | "settings";

export type TrayNavigatePayload = {
  page: AppPage;
};

export type NavigationItem = {
  id: AppPage;
  label: string;
  description: string;
};

export type ModelSource = "huggingface" | "lmstudio" | "ollama" | "custom";

export type ModelFormat = "gguf" | "safetensors" | "onnx" | "mlx" | "unknown";

export type DownloadStatus =
  | "queued"
  | "downloading"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type PageState = "empty" | "loading" | "ready" | "error";

export type AppSettings = {
  hfCachePath: string | null;
  lmStudioModelsPath: string | null;
  customModelFolders: string[];
  hfTokenStored: boolean;
  minimizeToTray: boolean;
  startOnLogin: boolean;
  enableSymlinkAttempt: boolean;
  scanOnStartup: boolean;
  deleteUsesRecycleBin: boolean;
  telemetryEnabled: false;
};

export type AppSettingsPatch = Partial<
  Omit<AppSettings, "hfTokenStored" | "telemetryEnabled">
>;

export type PathResolutionSource =
  | "user_setting"
  | "environment"
  | "default"
  | "unresolved";

export type PathIssueSeverity = "warning" | "error";

export type PathIssue = {
  severity: PathIssueSeverity;
  code: string;
  message: string;
};

export type ResolvedPath = {
  label: string;
  path: string | null;
  source: PathResolutionSource;
  sourceLabel: string;
  exists: boolean;
  isDirectory: boolean;
  issues: PathIssue[];
};

export type ResolvedPaths = {
  hfCache: ResolvedPath;
  lmStudioModels: ResolvedPath;
  customModelFolders: ResolvedPath[];
};

export type LocalModelFile = {
  name: string;
  path: string;
  sizeBytes: number | null;
  format: ModelFormat;
  quantization: string | null;
};

export type LocalModel = {
  id: string;
  displayName: string;
  provider: string | null;
  repoId: string | null;
  source: ModelSource;
  path: string | null;
  sizeBytes: number | null;
  format: ModelFormat | null;
  quantization: string | null;
  parameterSize: string | null;
  lastModified: string | null;
  files: LocalModelFile[];
  runtimeStatus: "available" | "loaded" | "running" | "unknown" | null;
};

export type DeleteModelInput = {
  id: string;
  source: ModelSource;
  path: string | null;
  repoId: string | null;
};

export type DeleteResult = {
  deletedPath: string;
  usedRecycleBin: boolean;
  message: string;
};

export type SourceStatus = {
  source: ModelSource;
  status: "ok" | "missing" | "error" | "disabled";
  path: string | null;
  message: string | null;
};

export type ScanResult = {
  models: LocalModel[];
  sourceStatuses: SourceStatus[];
  totalSizeBytes: number | null;
  scannedAt: string;
};

export type OllamaRuntimeStatus = {
  running: boolean;
  baseUrl: string;
  models: LocalModel[];
  error: string | null;
  checkedAt: string;
};

export type HfSearchSort = "downloads" | "likes" | "last_modified";

export type HfSearchFilters = {
  textGeneration: boolean;
  gguf: boolean;
  safetensors: boolean;
};

export type HfSearchInput = {
  query: string;
  filters: HfSearchFilters;
  sort: HfSearchSort;
  limit?: number;
};

export type HfFileSummary = {
  totalFiles: number;
  ggufFiles: number;
  safetensorsFiles: number;
  configFiles: number;
  tokenizerFiles: number;
};

export type HfModelSummary = {
  repoId: string;
  author: string | null;
  tags: string[];
  downloads: number | null;
  likes: number | null;
  lastModified: string | null;
  gated: boolean;
  private: boolean;
  pipelineTag: string | null;
  fileSummary: HfFileSummary;
};

export type HfSearchResult = {
  query: string;
  models: HfModelSummary[];
};

export type HfModelDetails = {
  repoId: string;
  revision: string;
  commitSha: string | null;
  gated: boolean;
  private: boolean;
  files: HfModelFile[];
  totalBytes: number | null;
};

export type HfModelFile = {
  path: string;
  sizeBytes: number | null;
  format: ModelFormat;
  extension: string | null;
  lfs: boolean;
  oid: string | null;
  blobId: string | null;
  likelyDefault: boolean;
};

export type DownloadDestination = "staging";

export type StartDownloadInput = {
  repoId: string;
  revision?: string | null;
  commitSha?: string | null;
  files: HfModelFile[];
  destination: DownloadDestination;
};

export type DownloadFileProgress = {
  path: string;
  sizeBytes: number | null;
  downloadedBytes: number;
  stagedPath: string | null;
  blobId: string | null;
  error: string | null;
};

export type DownloadJob = {
  id: string;
  repoId: string;
  revision: string;
  commitSha: string | null;
  destination: DownloadDestination;
  status: DownloadStatus;
  files: DownloadFileProgress[];
  totalBytes: number | null;
  downloadedBytes: number;
  error: string | null;
  installedAt: string | null;
  cachePath: string | null;
  snapshotPath: string | null;
  installError: string | null;
  installWarnings: string[];
  createdAt: string;
  updatedAt: string;
};

export type InstalledDownloadFile = {
  path: string;
  blobPath: string;
  snapshotPath: string;
  linked: boolean;
};

export type InstallDownloadResult = {
  jobId: string;
  repoId: string;
  cachePath: string;
  snapshotPath: string;
  installedFiles: InstalledDownloadFile[];
  warnings: string[];
};
