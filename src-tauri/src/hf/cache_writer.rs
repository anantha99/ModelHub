use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::models::{
    DownloadFileProgress, DownloadJob, DownloadStatus, InstallDownloadResult, InstalledDownloadFile,
};
use crate::scanner::huggingface::encode_repo_id;

pub fn install_download_to_cache(
    job: &DownloadJob,
    cache_root: &Path,
    enable_symlink_attempt: bool,
) -> Result<InstallDownloadResult, String> {
    validate_job_is_installable(job)?;

    let commit_sha = job
        .commit_sha
        .as_deref()
        .ok_or_else(|| {
            "ModelHub cannot install this download because Hugging Face did not return a commit SHA."
                .to_string()
        })
        .and_then(safe_path_segment)?;
    let repo_dir_name = encode_repo_id(&job.repo_id)
        .ok_or_else(|| "ModelHub could not encode this Hugging Face repo ID.".to_string())?;
    let repo_path = cache_root.join(repo_dir_name);
    let blobs_path = repo_path.join("blobs");
    let snapshots_path = repo_path.join("snapshots").join(commit_sha);
    let refs_path = repo_path.join("refs");
    let mut installed_files = Vec::new();
    let mut warnings = Vec::new();

    fs::create_dir_all(&blobs_path).map_err(|error| {
        format!(
            "ModelHub could not create Hugging Face blobs folder at {}: {error}",
            blobs_path.display()
        )
    })?;
    fs::create_dir_all(&snapshots_path).map_err(|error| {
        format!(
            "ModelHub could not create Hugging Face snapshot folder at {}: {error}",
            snapshots_path.display()
        )
    })?;
    fs::create_dir_all(&refs_path).map_err(|error| {
        format!(
            "ModelHub could not create Hugging Face refs folder at {}: {error}",
            refs_path.display()
        )
    })?;

    for file in &job.files {
        let installed_file = install_file(
            file,
            &blobs_path,
            &snapshots_path,
            enable_symlink_attempt,
            &mut warnings,
        )?;
        installed_files.push(installed_file);
    }

    write_ref(&refs_path, &job.revision, commit_sha, &mut warnings)?;

    Ok(InstallDownloadResult {
        job_id: job.id.clone(),
        repo_id: job.repo_id.clone(),
        cache_path: path_to_string(&repo_path),
        snapshot_path: path_to_string(&snapshots_path),
        installed_files,
        warnings,
    })
}

fn validate_job_is_installable(job: &DownloadJob) -> Result<(), String> {
    if job.status != DownloadStatus::Completed {
        return Err(
            "Only completed downloads can be installed into the Hugging Face cache.".to_string(),
        );
    }

    if job.files.is_empty() {
        return Err("This download has no files to install.".to_string());
    }

    Ok(())
}

fn install_file(
    file: &DownloadFileProgress,
    blobs_path: &Path,
    snapshots_path: &Path,
    enable_symlink_attempt: bool,
    warnings: &mut Vec<String>,
) -> Result<InstalledDownloadFile, String> {
    let staged_path = file
        .staged_path
        .as_deref()
        .ok_or_else(|| format!("{} was not staged and cannot be installed.", file.path))?;
    let staged_path = PathBuf::from(staged_path);

    if !staged_path.is_file() {
        return Err(format!(
            "ModelHub could not find staged file {} for {}.",
            staged_path.display(),
            file.path
        ));
    }

    let blob_id = file
        .blob_id
        .as_deref()
        .ok_or_else(|| {
            format!(
                "{} cannot be installed because Hugging Face did not provide a blob ID.",
                file.path
            )
        })
        .and_then(safe_path_segment)?;
    let relative_path = safe_relative_path(&file.path)?;
    let blob_path = blobs_path.join(blob_id);
    let snapshot_file_path = snapshots_path.join(relative_path);

    install_blob(&staged_path, &blob_path, file)?;

    if let Some(parent) = snapshot_file_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "ModelHub could not create snapshot folder {}: {error}",
                parent.display()
            )
        })?;
    }

    let linked = install_snapshot_entry(
        &blob_path,
        &snapshot_file_path,
        enable_symlink_attempt,
        &file.path,
        warnings,
    )?;

    Ok(InstalledDownloadFile {
        path: file.path.clone(),
        blob_path: path_to_string(&blob_path),
        snapshot_path: path_to_string(&snapshot_file_path),
        linked,
    })
}

fn install_blob(
    staged_path: &Path,
    blob_path: &Path,
    file: &DownloadFileProgress,
) -> Result<(), String> {
    let staged_size = fs::metadata(staged_path)
        .map_err(|error| {
            format!(
                "ModelHub could not inspect {}: {error}",
                staged_path.display()
            )
        })?
        .len();

    if let Some(expected_size) = file.size_bytes {
        if staged_size != expected_size {
            return Err(format!(
                "Staged size for {} changed before install. Expected {} bytes, got {} bytes.",
                file.path, expected_size, staged_size
            ));
        }
    }

    if blob_path.exists() {
        let existing_size = fs::metadata(blob_path)
            .map_err(|error| {
                format!(
                    "ModelHub could not inspect {}: {error}",
                    blob_path.display()
                )
            })?
            .len();

        if existing_size == staged_size {
            return Ok(());
        }

        return Err(format!(
            "A different blob already exists at {}. ModelHub will not overwrite it.",
            blob_path.display()
        ));
    }

    let temporary_path = blob_path.with_extension("tmp");
    fs::copy(staged_path, &temporary_path).map_err(|error| {
        format!(
            "ModelHub could not copy staged file {} to {}: {error}",
            staged_path.display(),
            temporary_path.display()
        )
    })?;
    fs::rename(&temporary_path, blob_path).map_err(|error| {
        format!(
            "ModelHub could not finalize blob {}: {error}",
            blob_path.display()
        )
    })
}

fn install_snapshot_entry(
    blob_path: &Path,
    snapshot_file_path: &Path,
    enable_symlink_attempt: bool,
    file_path: &str,
    warnings: &mut Vec<String>,
) -> Result<bool, String> {
    remove_existing_snapshot_file(snapshot_file_path)?;

    if enable_symlink_attempt {
        match symlink_file(blob_path, snapshot_file_path) {
            Ok(()) => return Ok(true),
            Err(error) => warnings.push(format!(
                "Could not create snapshot symlink for {file_path}; copied the file instead. {error}"
            )),
        }
    }

    fs::copy(blob_path, snapshot_file_path).map_err(|error| {
        format!(
            "ModelHub could not copy blob {} to snapshot {}: {error}",
            blob_path.display(),
            snapshot_file_path.display()
        )
    })?;

    Ok(false)
}

fn remove_existing_snapshot_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Err(format!(
                    "ModelHub will not replace existing snapshot folder {}.",
                    path.display()
                ));
            }

            fs::remove_file(path).map_err(|error| {
                format!(
                    "ModelHub could not replace existing snapshot file {}: {error}",
                    path.display()
                )
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "ModelHub could not inspect snapshot file {}: {error}",
            path.display()
        )),
    }
}

fn write_ref(
    refs_path: &Path,
    revision: &str,
    commit_sha: &str,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(ref_name) = safe_ref_name(revision) else {
        warnings.push(format!(
            "Skipped refs entry for revision {revision}; only simple revision names are supported."
        ));
        return Ok(());
    };
    let ref_path = refs_path.join(ref_name);
    let temporary_path = ref_path.with_extension("tmp");

    fs::write(&temporary_path, commit_sha).map_err(|error| {
        format!(
            "ModelHub could not write Hugging Face ref {}: {error}",
            temporary_path.display()
        )
    })?;
    fs::rename(&temporary_path, &ref_path).map_err(|error| {
        format!(
            "ModelHub could not finalize Hugging Face ref {}: {error}",
            ref_path.display()
        )
    })
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
            _ => return Err(format!("ModelHub rejected an unsafe download path: {path}")),
        }
    }

    if relative.as_os_str().is_empty() {
        Err("Download file paths cannot be empty.".to_string())
    } else {
        Ok(relative)
    }
}

fn safe_path_segment(value: &str) -> Result<&str, String> {
    let value = value.trim();

    if value.is_empty()
        || value == "."
        || value == ".."
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        Err("ModelHub rejected an unsafe Hugging Face cache path segment.".to_string())
    } else {
        Ok(value)
    }
}

fn safe_ref_name(value: &str) -> Option<&str> {
    safe_path_segment(value).ok()
}

#[cfg(windows)]
fn symlink_file(original: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(original, link)
}

#[cfg(not(windows))]
fn symlink_file(original: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DownloadDestination, DownloadFileProgress};

    fn completed_job(staged_file: &Path) -> DownloadJob {
        DownloadJob {
            id: "download-1".to_string(),
            repo_id: "Qwen/Qwen3-4B".to_string(),
            revision: "main".to_string(),
            commit_sha: Some("abc123".to_string()),
            destination: DownloadDestination::Staging,
            status: DownloadStatus::Completed,
            files: vec![DownloadFileProgress {
                path: "nested/config.json".to_string(),
                size_bytes: Some(5),
                downloaded_bytes: 5,
                staged_path: Some(path_to_string(staged_file)),
                blob_id: Some("blob-a".to_string()),
                error: None,
            }],
            total_bytes: Some(5),
            downloaded_bytes: 5,
            error: None,
            installed_at: None,
            cache_path: None,
            snapshot_path: None,
            install_error: None,
            install_warnings: Vec::new(),
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
        }
    }

    #[test]
    fn installs_completed_download_into_hf_cache_layout() {
        let directory = tempfile::tempdir().expect("temp dir");
        let staged_file = directory.path().join("staged-config.json");
        fs::write(&staged_file, b"hello").expect("write staged file");

        let result = install_download_to_cache(
            &completed_job(&staged_file),
            directory.path().join("hub").as_path(),
            false,
        )
        .expect("install should succeed");

        let repo_path = directory.path().join("hub").join("models--Qwen--Qwen3-4B");
        assert_eq!(result.cache_path, path_to_string(&repo_path));
        assert_eq!(
            fs::read(repo_path.join("blobs").join("blob-a")).unwrap(),
            b"hello"
        );
        assert_eq!(
            fs::read(
                repo_path
                    .join("snapshots")
                    .join("abc123")
                    .join("nested")
                    .join("config.json")
            )
            .unwrap(),
            b"hello"
        );
        assert_eq!(
            fs::read_to_string(repo_path.join("refs").join("main")).unwrap(),
            "abc123"
        );
        assert_eq!(result.installed_files.len(), 1);
        assert!(!result.installed_files[0].linked);
    }

    #[test]
    fn refuses_missing_commit_sha() {
        let directory = tempfile::tempdir().expect("temp dir");
        let staged_file = directory.path().join("staged-config.json");
        fs::write(&staged_file, b"hello").expect("write staged file");
        let mut job = completed_job(&staged_file);
        job.commit_sha = None;

        let error = install_download_to_cache(&job, directory.path(), false)
            .expect_err("install should fail");

        assert!(error.contains("commit SHA"));
    }

    #[test]
    fn refuses_unsafe_file_paths() {
        let directory = tempfile::tempdir().expect("temp dir");
        let staged_file = directory.path().join("staged-config.json");
        fs::write(&staged_file, b"hello").expect("write staged file");
        let mut job = completed_job(&staged_file);
        job.files[0].path = "../config.json".to_string();

        let error = install_download_to_cache(&job, directory.path(), false)
            .expect_err("install should fail");

        assert!(error.contains("unsafe"));
    }

    #[test]
    fn refuses_missing_blob_id() {
        let directory = tempfile::tempdir().expect("temp dir");
        let staged_file = directory.path().join("staged-config.json");
        fs::write(&staged_file, b"hello").expect("write staged file");
        let mut job = completed_job(&staged_file);
        job.files[0].blob_id = None;

        let error = install_download_to_cache(&job, directory.path(), false)
            .expect_err("install should fail");

        assert!(error.contains("blob ID"));
    }

    #[test]
    fn refuses_to_overwrite_mismatched_existing_blob() {
        let directory = tempfile::tempdir().expect("temp dir");
        let staged_file = directory.path().join("staged-config.json");
        fs::write(&staged_file, b"hello").expect("write staged file");
        let existing_blob = directory
            .path()
            .join("hub")
            .join("models--Qwen--Qwen3-4B")
            .join("blobs")
            .join("blob-a");
        fs::create_dir_all(existing_blob.parent().unwrap()).expect("create blob parent");
        fs::write(&existing_blob, b"different").expect("write existing blob");

        let error = install_download_to_cache(
            &completed_job(&staged_file),
            directory.path().join("hub").as_path(),
            false,
        )
        .expect_err("install should fail");

        assert!(error.contains("will not overwrite"));
    }
}
