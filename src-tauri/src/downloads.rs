use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::Url;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::hf::cache_writer;
use crate::models::{
    DownloadFileProgress, DownloadJob, DownloadStatus, HfModelFile, InstallDownloadResult,
    StartDownloadInput,
};

const DOWNLOADS_FILE_NAME: &str = "downloads.json";
const DOWNLOADS_DIR_NAME: &str = "downloads";
const HF_BASE_URL: &str = "https://huggingface.co";
const BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct DownloadManager {
    inner: Arc<Mutex<DownloadManagerInner>>,
}

#[derive(Debug)]
struct DownloadManagerInner {
    root: PathBuf,
    metadata_path: PathBuf,
    jobs: Vec<DownloadJob>,
    cancelled_jobs: HashSet<String>,
}

impl DownloadManager {
    pub fn for_app<R: Runtime>(app: &tauri::App<R>) -> Result<Self, String> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("ModelHub could not resolve its app data folder: {error}"))?;
        Self::new(app_data_dir.join(DOWNLOADS_DIR_NAME))
    }

    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|error| {
            format!(
                "ModelHub could not create the downloads folder at {}: {error}",
                root.display()
            )
        })?;

        let metadata_path = root.join(DOWNLOADS_FILE_NAME);
        let mut jobs = load_jobs(&metadata_path)?;
        let mut changed = false;

        for job in &mut jobs {
            if matches!(
                job.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            ) {
                job.status = DownloadStatus::Failed;
                job.error = Some(
                    "ModelHub restarted before this download finished. Restart the download to try again."
                        .to_string(),
                );
                job.updated_at = timestamp();
                changed = true;
            }
        }

        let manager = Self {
            inner: Arc::new(Mutex::new(DownloadManagerInner {
                root,
                metadata_path,
                jobs,
                cancelled_jobs: HashSet::new(),
            })),
        };

        if changed {
            manager.save_jobs()?;
        }

        Ok(manager)
    }

    pub fn list_jobs(&self) -> Result<Vec<DownloadJob>, String> {
        let inner = self.lock()?;
        Ok(inner.jobs.clone())
    }

    pub fn start_download(
        &self,
        app: AppHandle,
        input: StartDownloadInput,
    ) -> Result<DownloadJob, String> {
        validate_start_input(&input)?;

        let job_id = new_job_id();
        let created_at = timestamp();
        let revision = input
            .revision
            .as_deref()
            .and_then(trimmed_non_empty)
            .unwrap_or("main")
            .to_string();
        let total_bytes = sum_known_file_sizes(&input.files);
        let job = DownloadJob {
            id: job_id.clone(),
            repo_id: input.repo_id.trim().to_string(),
            revision,
            commit_sha: input
                .commit_sha
                .as_deref()
                .and_then(trimmed_non_empty)
                .map(str::to_string),
            destination: input.destination,
            status: DownloadStatus::Queued,
            files: input
                .files
                .iter()
                .map(|file| DownloadFileProgress {
                    path: file.path.clone(),
                    size_bytes: file.size_bytes,
                    downloaded_bytes: 0,
                    staged_path: None,
                    blob_id: file.blob_id.clone(),
                    error: None,
                })
                .collect(),
            total_bytes,
            downloaded_bytes: 0,
            error: None,
            installed_at: None,
            cache_path: None,
            snapshot_path: None,
            install_error: None,
            install_warnings: Vec::new(),
            created_at: created_at.clone(),
            updated_at: created_at,
        };

        {
            let mut inner = self.lock()?;
            inner.cancelled_jobs.remove(&job_id);
            inner.jobs.push(job.clone());
            save_jobs_to_path(&inner.metadata_path, &inner.jobs)?;
        }

        emit_event(&app, "download:updated", &job);

        let manager = self.clone();
        tauri::async_runtime::spawn_blocking(move || {
            manager.run_download(app, job_id);
        });

        Ok(job)
    }

    pub fn cancel_download(&self, app: AppHandle, job_id: String) -> Result<(), String> {
        let maybe_job = {
            let mut inner = self.lock()?;
            inner.cancelled_jobs.insert(job_id.clone());

            let Some(job) = inner.jobs.iter_mut().find(|job| job.id == job_id) else {
                return Err("ModelHub could not find that download job.".to_string());
            };

            if matches!(
                job.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            ) {
                job.status = DownloadStatus::Cancelled;
                job.error = None;
                job.updated_at = timestamp();
                let job = job.clone();
                save_jobs_to_path(&inner.metadata_path, &inner.jobs)?;
                Some(job)
            } else {
                None
            }
        };

        if let Some(job) = maybe_job {
            emit_event(&app, "download:updated", &job);
        }

        Ok(())
    }

    pub fn unsupported_resume_control(&self) -> Result<(), String> {
        Err("Pause and resume require HTTP range support and are not implemented yet.".to_string())
    }

    pub fn install_download(
        &self,
        app: AppHandle,
        job_id: String,
        cache_root: PathBuf,
        enable_symlink_attempt: bool,
    ) -> Result<InstallDownloadResult, String> {
        let job = self
            .job_snapshot(&job_id)?
            .ok_or_else(|| "ModelHub could not find that download job.".to_string())?;

        match cache_writer::install_download_to_cache(&job, &cache_root, enable_symlink_attempt) {
            Ok(result) => {
                if let Some(updated_job) = self.mark_installed(&job_id, &result) {
                    emit_event(&app, "download:updated", &updated_job);
                }
                Ok(result)
            }
            Err(error) => {
                if let Some(updated_job) = self.mark_install_failed(&job_id, &error) {
                    emit_event(&app, "download:updated", &updated_job);
                }
                Err(error)
            }
        }
    }

    fn run_download(&self, app: AppHandle, job_id: String) {
        if let Some(job) = self.set_job_status(&job_id, DownloadStatus::Downloading, None) {
            emit_event(&app, "download:updated", &job);
        }

        let result = self.transfer_job(&app, &job_id);

        match result {
            Ok(DownloadTerminalStatus::Completed) => {
                if let Some(job) = self.set_job_status(&job_id, DownloadStatus::Completed, None) {
                    emit_event(&app, "download:updated", &job);
                    emit_event(&app, "download:completed", &job);
                }
            }
            Ok(DownloadTerminalStatus::Cancelled) => {
                if let Some(job) = self.set_job_status(&job_id, DownloadStatus::Cancelled, None) {
                    emit_event(&app, "download:updated", &job);
                }
            }
            Err(error) => {
                if let Some(job) = self.set_job_status(&job_id, DownloadStatus::Failed, Some(error))
                {
                    emit_event(&app, "download:updated", &job);
                    emit_event(&app, "download:failed", &job);
                }
            }
        }
    }

    fn transfer_job(
        &self,
        app: &AppHandle,
        job_id: &str,
    ) -> Result<DownloadTerminalStatus, String> {
        let client = Client::builder()
            .user_agent("ModelHub-Windows/0.1")
            .build()
            .map_err(|error| format!("ModelHub could not prepare the download client: {error}"))?;
        let job = self
            .job_snapshot(job_id)?
            .ok_or_else(|| "ModelHub could not find that download job.".to_string())?;

        for file_index in 0..job.files.len() {
            if self.is_cancelled(job_id)? {
                return Ok(DownloadTerminalStatus::Cancelled);
            }

            let file = job.files[file_index].clone();
            let download_url = build_file_url(&job.repo_id, &job.revision, &file.path)?;
            let mut response = client
                .get(download_url)
                .send()
                .map_err(|error| format!("Could not download {}: {error}", file.path))?;

            if !response.status().is_success() {
                return Err(format!(
                    "Hugging Face returned HTTP {} while downloading {}.",
                    response.status().as_u16(),
                    file.path
                ));
            }

            let (part_path, final_path) = self.staging_paths(job_id, &file.path)?;
            if let Some(parent) = part_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Could not create staging folder {}: {error}",
                        parent.display()
                    )
                })?;
            }

            let mut output = File::create(&part_path).map_err(|error| {
                format!(
                    "Could not create partial download {}: {error}",
                    part_path.display()
                )
            })?;
            let mut buffer = vec![0_u8; BUFFER_SIZE];

            loop {
                if self.is_cancelled(job_id)? {
                    let _ = fs::remove_file(&part_path);
                    return Ok(DownloadTerminalStatus::Cancelled);
                }

                let read = response
                    .read(&mut buffer)
                    .map_err(|error| format!("Could not read data for {}: {error}", file.path))?;

                if read == 0 {
                    break;
                }

                output
                    .write_all(&buffer[..read])
                    .map_err(|error| format!("Could not write data for {}: {error}", file.path))?;

                if let Some(updated_job) = self.add_file_progress(job_id, file_index, read as u64) {
                    emit_event(app, "download:updated", &updated_job);
                }
            }

            output
                .flush()
                .map_err(|error| format!("Could not finish writing {}: {error}", file.path))?;

            if let Some(expected_size) = file.size_bytes {
                let actual_size = fs::metadata(&part_path)
                    .map_err(|error| format!("Could not verify {}: {error}", file.path))?
                    .len();

                if actual_size != expected_size {
                    return Err(format!(
                        "Downloaded size for {} did not match Hugging Face metadata. Expected {} bytes, got {} bytes.",
                        file.path, expected_size, actual_size
                    ));
                }
            }

            if final_path.exists() {
                fs::remove_file(&final_path).map_err(|error| {
                    format!(
                        "Could not replace staged file {}: {error}",
                        final_path.display()
                    )
                })?;
            }

            fs::rename(&part_path, &final_path).map_err(|error| {
                format!(
                    "Could not finalize staged file {}: {error}",
                    final_path.display()
                )
            })?;

            if let Some(updated_job) = self.finish_file(job_id, file_index, &final_path) {
                emit_event(app, "download:updated", &updated_job);
            }
        }

        Ok(DownloadTerminalStatus::Completed)
    }

    fn staging_paths(&self, job_id: &str, file_path: &str) -> Result<(PathBuf, PathBuf), String> {
        let relative_path = safe_relative_path(file_path)?;
        let root = {
            let inner = self.lock()?;
            inner.root.clone()
        };
        let final_path = root.join(job_id).join(relative_path);
        let part_path = PathBuf::from(format!("{}.part", final_path.display()));

        Ok((part_path, final_path))
    }

    fn job_snapshot(&self, job_id: &str) -> Result<Option<DownloadJob>, String> {
        let inner = self.lock()?;
        Ok(inner.jobs.iter().find(|job| job.id == job_id).cloned())
    }

    fn is_cancelled(&self, job_id: &str) -> Result<bool, String> {
        let inner = self.lock()?;
        Ok(inner.cancelled_jobs.contains(job_id)
            || inner
                .jobs
                .iter()
                .find(|job| job.id == job_id)
                .map(|job| job.status == DownloadStatus::Cancelled)
                .unwrap_or(false))
    }

    fn set_job_status(
        &self,
        job_id: &str,
        status: DownloadStatus,
        error: Option<String>,
    ) -> Option<DownloadJob> {
        let mut inner = self.lock().ok()?;
        let job = inner.jobs.iter_mut().find(|job| job.id == job_id)?;
        job.status = status;
        job.error = error;
        job.updated_at = timestamp();
        let job = job.clone();
        let _ = save_jobs_to_path(&inner.metadata_path, &inner.jobs);
        Some(job)
    }

    fn add_file_progress(
        &self,
        job_id: &str,
        file_index: usize,
        bytes: u64,
    ) -> Option<DownloadJob> {
        let mut inner = self.lock().ok()?;
        let job = inner.jobs.iter_mut().find(|job| job.id == job_id)?;
        let file = job.files.get_mut(file_index)?;
        file.downloaded_bytes = file.downloaded_bytes.saturating_add(bytes);
        job.downloaded_bytes = job.downloaded_bytes.saturating_add(bytes);
        job.updated_at = timestamp();
        let job = job.clone();
        let _ = save_jobs_to_path(&inner.metadata_path, &inner.jobs);
        Some(job)
    }

    fn finish_file(
        &self,
        job_id: &str,
        file_index: usize,
        final_path: &Path,
    ) -> Option<DownloadJob> {
        let mut inner = self.lock().ok()?;
        let job = inner.jobs.iter_mut().find(|job| job.id == job_id)?;
        let file = job.files.get_mut(file_index)?;
        file.staged_path = Some(final_path.to_string_lossy().to_string());
        file.error = None;
        job.updated_at = timestamp();
        let job = job.clone();
        let _ = save_jobs_to_path(&inner.metadata_path, &inner.jobs);
        Some(job)
    }

    fn mark_installed(&self, job_id: &str, result: &InstallDownloadResult) -> Option<DownloadJob> {
        let mut inner = self.lock().ok()?;
        let job = inner.jobs.iter_mut().find(|job| job.id == job_id)?;
        let now = timestamp();
        job.installed_at = Some(now.clone());
        job.cache_path = Some(result.cache_path.clone());
        job.snapshot_path = Some(result.snapshot_path.clone());
        job.install_error = None;
        job.install_warnings = result.warnings.clone();
        job.updated_at = now;
        let job = job.clone();
        let _ = save_jobs_to_path(&inner.metadata_path, &inner.jobs);
        Some(job)
    }

    fn mark_install_failed(&self, job_id: &str, error: &str) -> Option<DownloadJob> {
        let mut inner = self.lock().ok()?;
        let job = inner.jobs.iter_mut().find(|job| job.id == job_id)?;
        job.install_error = Some(error.to_string());
        job.updated_at = timestamp();
        let job = job.clone();
        let _ = save_jobs_to_path(&inner.metadata_path, &inner.jobs);
        Some(job)
    }

    fn save_jobs(&self) -> Result<(), String> {
        let inner = self.lock()?;
        save_jobs_to_path(&inner.metadata_path, &inner.jobs)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, DownloadManagerInner>, String> {
        self.inner
            .lock()
            .map_err(|_| "ModelHub download state is unavailable.".to_string())
    }
}

enum DownloadTerminalStatus {
    Completed,
    Cancelled,
}

fn load_jobs(path: &Path) -> Result<Vec<DownloadJob>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "ModelHub could not read saved downloads from {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "ModelHub could not read saved downloads from {} because the file is not valid JSON: {error}",
            path.display()
        )
    })
}

fn save_jobs_to_path(path: &Path, jobs: &[DownloadJob]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("ModelHub could not create {}: {error}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(jobs)
        .map_err(|error| format!("ModelHub could not save download metadata: {error}"))?;
    let temporary_path = path.with_extension("json.tmp");

    fs::write(&temporary_path, contents).map_err(|error| {
        format!(
            "ModelHub could not write download metadata to {}: {error}",
            temporary_path.display()
        )
    })?;

    fs::rename(&temporary_path, path).or_else(|rename_error| {
        if path.exists() {
            fs::remove_file(path).map_err(|error| {
                format!(
                    "ModelHub could not replace download metadata at {}: {error}",
                    path.display()
                )
            })?;
            fs::rename(&temporary_path, path).map_err(|error| {
                format!(
                    "ModelHub could not save download metadata to {}: {error}",
                    path.display()
                )
            })
        } else {
            Err(format!(
                "ModelHub could not save download metadata to {}: {rename_error}",
                path.display()
            ))
        }
    })
}

fn validate_start_input(input: &StartDownloadInput) -> Result<(), String> {
    let repo_id = input.repo_id.trim();

    if repo_id.is_empty() || !repo_id.contains('/') || repo_id.contains("..") {
        return Err("Choose a valid Hugging Face repo before starting a download.".to_string());
    }

    if input.files.is_empty() {
        return Err("Select at least one file to download.".to_string());
    }

    for file in &input.files {
        safe_relative_path(&file.path)?;
    }

    Ok(())
}

fn safe_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = path.trim().replace('\\', "/");

    if path.is_empty() {
        return Err("Download file paths cannot be empty.".to_string());
    }

    let mut relative = PathBuf::new();

    for component in Path::new(&path).components() {
        match component {
            Component::Normal(value) => relative.push(value),
            _ => {
                return Err(format!("ModelHub rejected an unsafe download path: {path}"));
            }
        }
    }

    if relative.as_os_str().is_empty() {
        Err("Download file paths cannot be empty.".to_string())
    } else {
        Ok(relative)
    }
}

fn build_file_url(repo_id: &str, revision: &str, file_path: &str) -> Result<Url, String> {
    let mut url = Url::parse(HF_BASE_URL).map_err(|error| error.to_string())?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "ModelHub could not build the Hugging Face download URL.".to_string())?;

        for segment in repo_id.split('/') {
            segments.push(segment);
        }

        segments.push("resolve");
        segments.push(revision);

        for segment in file_path.replace('\\', "/").split('/') {
            if !segment.is_empty() {
                segments.push(segment);
            }
        }
    }

    Ok(url)
}

fn sum_known_file_sizes(files: &[HfModelFile]) -> Option<u64> {
    let mut total = 0_u64;

    for file in files {
        total = total.checked_add(file.size_bytes?)?;
    }

    Some(total)
}

fn new_job_id() -> String {
    format!("download-{}", unix_millis())
}

fn timestamp() -> String {
    unix_millis().to_string()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn trimmed_non_empty(value: &str) -> Option<&str> {
    let value = value.trim();

    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn emit_event(app: &AppHandle, event: &str, job: &DownloadJob) {
    let _ = app.emit(event, job);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DownloadDestination, ModelFormat};

    fn hf_file(path: &str, size_bytes: Option<u64>) -> HfModelFile {
        HfModelFile {
            path: path.to_string(),
            size_bytes,
            format: ModelFormat::Unknown,
            extension: None,
            lfs: false,
            oid: None,
            blob_id: None,
            likely_default: false,
        }
    }

    #[test]
    fn rejects_unsafe_download_paths() {
        assert!(safe_relative_path("model.gguf").is_ok());
        assert!(safe_relative_path("nested/model.gguf").is_ok());
        assert!(safe_relative_path("../secret.txt").is_err());
        assert!(safe_relative_path("C:/secret.txt").is_err());
    }

    #[test]
    fn validates_start_input_requires_files_and_repo() {
        let mut input = StartDownloadInput {
            repo_id: "Qwen/Qwen3-4B".to_string(),
            revision: None,
            commit_sha: None,
            files: vec![hf_file("config.json", Some(12))],
            destination: DownloadDestination::Staging,
        };

        assert!(validate_start_input(&input).is_ok());

        input.files.clear();
        assert!(validate_start_input(&input).is_err());

        input.files.push(hf_file("../config.json", Some(12)));
        assert!(validate_start_input(&input).is_err());
    }

    #[test]
    fn sums_file_sizes_only_when_all_are_known() {
        assert_eq!(
            sum_known_file_sizes(&[hf_file("a", Some(5)), hf_file("b", Some(7))]),
            Some(12)
        );
        assert_eq!(
            sum_known_file_sizes(&[hf_file("a", Some(5)), hf_file("b", None)]),
            None
        );
    }
}
