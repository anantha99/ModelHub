use tauri::AppHandle;

use crate::downloads::DownloadManager;
use crate::errors::{into_command_error, CommandResult};
use crate::hf;
use crate::model_actions;
use crate::models::{
    AppSettings, AppSettingsPatch, DeleteModelInput, DeleteResult, DownloadJob, HfModelDetails,
    HfSearchInput, HfSearchResult, InstallDownloadResult, OllamaRuntimeStatus, ResolvedPaths,
    ScanResult, StartDownloadInput, SystemInfo,
};
use crate::paths;
use crate::runtimes;
use crate::scanner;
use crate::settings::SettingsStore;
use crate::system_info;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> CommandResult<AppSettings> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    store.load().map_err(into_command_error)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, patch: AppSettingsPatch) -> CommandResult<AppSettings> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    store.update(patch).map_err(into_command_error)
}

#[tauri::command]
pub fn get_resolved_paths(app: AppHandle) -> CommandResult<ResolvedPaths> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    let settings = store.load().map_err(into_command_error)?;

    Ok(paths::resolve_paths(&settings))
}

#[tauri::command]
pub async fn scan_models(app: AppHandle) -> CommandResult<ScanResult> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    let settings = store.load().map_err(into_command_error)?;
    let resolved_paths = paths::resolve_paths(&settings);

    tauri::async_runtime::spawn_blocking(move || scanner::scan_models(&resolved_paths))
        .await
        .map_err(|error| format!("Model scan failed to finish: {error}"))
}

#[tauri::command]
pub async fn get_ollama_runtime_status() -> CommandResult<OllamaRuntimeStatus> {
    tauri::async_runtime::spawn_blocking(runtimes::ollama::get_status)
        .await
        .map_err(|error| format!("Ollama runtime check failed to finish: {error}"))
}

#[tauri::command]
pub async fn get_system_info(app: AppHandle) -> CommandResult<SystemInfo> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    let settings = store.load().map_err(into_command_error)?;
    let resolved_paths = paths::resolve_paths(&settings);
    let hf_cache_path = resolved_paths.hf_cache.path;

    tauri::async_runtime::spawn_blocking(move || system_info::collect_system_info(hf_cache_path))
        .await
        .map_err(|error| format!("System information check failed to finish: {error}"))
}

#[tauri::command]
pub async fn search_hf_models(input: HfSearchInput) -> CommandResult<HfSearchResult> {
    tauri::async_runtime::spawn_blocking(move || {
        hf::api::search_models(input).map_err(|error| error.user_message())
    })
    .await
    .map_err(|error| format!("Hugging Face search failed to finish: {error}"))?
}

#[tauri::command]
pub async fn get_hf_model_details(
    repo_id: String,
    revision: Option<String>,
) -> CommandResult<HfModelDetails> {
    tauri::async_runtime::spawn_blocking(move || {
        hf::api::get_model_details(repo_id, revision).map_err(|error| error.user_message())
    })
    .await
    .map_err(|error| format!("Hugging Face model details failed to finish: {error}"))?
}

#[tauri::command]
pub fn start_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    input: StartDownloadInput,
) -> CommandResult<DownloadJob> {
    manager.start_download(app, input)
}

#[tauri::command]
pub fn list_downloads(
    manager: tauri::State<'_, DownloadManager>,
) -> CommandResult<Vec<DownloadJob>> {
    manager.list_jobs()
}

#[tauri::command]
pub fn cancel_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    job_id: String,
) -> CommandResult<()> {
    manager.cancel_download(app, job_id)
}

#[tauri::command]
pub fn pause_download(
    manager: tauri::State<'_, DownloadManager>,
    _job_id: String,
) -> CommandResult<()> {
    manager.unsupported_resume_control()
}

#[tauri::command]
pub fn resume_download(
    manager: tauri::State<'_, DownloadManager>,
    _job_id: String,
) -> CommandResult<()> {
    manager.unsupported_resume_control()
}

#[tauri::command]
pub fn install_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    job_id: String,
) -> CommandResult<InstallDownloadResult> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    let settings = store.load().map_err(into_command_error)?;
    let resolved_paths = paths::resolve_paths(&settings);
    let cache_path = resolved_paths
        .hf_cache
        .path
        .ok_or_else(|| "ModelHub could not resolve the Hugging Face cache path.".to_string())?;

    manager.install_download(
        app,
        job_id,
        std::path::PathBuf::from(cache_path),
        settings.enable_symlink_attempt,
    )
}

#[tauri::command]
pub fn open_path(path: String) -> CommandResult<()> {
    model_actions::open_path(&path)
}

#[tauri::command]
pub fn delete_model(app: AppHandle, input: DeleteModelInput) -> CommandResult<DeleteResult> {
    let store = SettingsStore::for_manager(&app).map_err(into_command_error)?;
    let settings = store.load().map_err(into_command_error)?;
    let resolved_paths = paths::resolve_paths(&settings);

    model_actions::delete_model(input, &resolved_paths)
}
