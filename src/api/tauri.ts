import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  AppSettingsPatch,
  DeleteModelInput,
  DeleteResult,
  DownloadJob,
  HfModelDetails,
  HfSearchInput,
  HfSearchResult,
  InstallDownloadResult,
  OllamaRuntimeStatus,
  ResolvedPaths,
  ScanResult,
  StartDownloadInput,
} from "./types";

type CommandArgs = Record<string, unknown>;

export function invokeCommand<Result>(
  command: string,
  args?: CommandArgs,
): Promise<Result> {
  return invoke<Result>(command, args);
}

export function getSettings(): Promise<AppSettings> {
  return invokeCommand<AppSettings>("get_settings");
}

export function updateSettings(
  patch: AppSettingsPatch,
): Promise<AppSettings> {
  return invokeCommand<AppSettings>("update_settings", { patch });
}

export function getResolvedPaths(): Promise<ResolvedPaths> {
  return invokeCommand<ResolvedPaths>("get_resolved_paths");
}

export function scanModels(): Promise<ScanResult> {
  return invokeCommand<ScanResult>("scan_models");
}

export function getOllamaRuntimeStatus(): Promise<OllamaRuntimeStatus> {
  return invokeCommand<OllamaRuntimeStatus>("get_ollama_runtime_status");
}

export function searchHfModels(input: HfSearchInput): Promise<HfSearchResult> {
  return invokeCommand<HfSearchResult>("search_hf_models", { input });
}

export function getHfModelDetails(
  repoId: string,
  revision?: string,
): Promise<HfModelDetails> {
  return invokeCommand<HfModelDetails>("get_hf_model_details", { repoId, revision });
}

export function startDownload(input: StartDownloadInput): Promise<DownloadJob> {
  return invokeCommand<DownloadJob>("start_download", { input });
}

export function listDownloads(): Promise<DownloadJob[]> {
  return invokeCommand<DownloadJob[]>("list_downloads");
}

export function cancelDownload(jobId: string): Promise<void> {
  return invokeCommand<void>("cancel_download", { jobId });
}

export function installDownload(jobId: string): Promise<InstallDownloadResult> {
  return invokeCommand<InstallDownloadResult>("install_download", { jobId });
}

export function pauseDownload(jobId: string): Promise<void> {
  return invokeCommand<void>("pause_download", { jobId });
}

export function resumeDownload(jobId: string): Promise<void> {
  return invokeCommand<void>("resume_download", { jobId });
}

export function openPath(path: string): Promise<void> {
  return invokeCommand<void>("open_path", { path });
}

export function deleteModel(input: DeleteModelInput): Promise<DeleteResult> {
  return invokeCommand<DeleteResult>("delete_model", { input });
}
