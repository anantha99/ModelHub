use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{LocalModel, LocalModelFile, ModelFormat, ModelRuntimeStatus, ModelSource};

use super::common::{detect_format, parse_quantization_from_name};

const MODEL_CACHE_PREFIX: &str = "models--";

pub struct HfScanReport {
    pub models: Vec<LocalModel>,
    pub warnings: Vec<String>,
}

pub fn scan_cache(cache_root: &Path) -> Result<HfScanReport, String> {
    let entries = fs::read_dir(cache_root).map_err(|error| {
        format!(
            "Could not read Hugging Face cache folder at {}: {error}",
            cache_root.display()
        )
    })?;
    let mut models = Vec::new();
    let mut warnings = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!("Could not read a cache entry: {error}."));
                continue;
            }
        };

        let entry_path = entry.path();

        if !entry_path.is_dir() {
            continue;
        }

        let entry_name = entry.file_name().to_string_lossy().to_string();
        let Some(repo_id) = decode_cache_repo_dir_name(&entry_name) else {
            continue;
        };

        match scan_repo_cache(&entry_path, &repo_id) {
            Ok(Some(model)) => models.push(model),
            Ok(None) => warnings.push(format!("{repo_id} has no readable snapshots.")),
            Err(error) => warnings.push(format!("{repo_id}: {error}")),
        }
    }

    models.sort_by(|left, right| left.display_name.cmp(&right.display_name));

    Ok(HfScanReport { models, warnings })
}

#[allow(dead_code)]
pub fn encode_repo_id(repo_id: &str) -> Option<String> {
    let repo_id = repo_id.trim();
    let (provider, name) = repo_id.split_once('/')?;

    if provider.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }

    Some(format!("{MODEL_CACHE_PREFIX}{provider}--{name}"))
}

pub fn decode_cache_repo_dir_name(name: &str) -> Option<String> {
    let encoded = name.strip_prefix(MODEL_CACHE_PREFIX)?;
    let (provider, repo_name) = encoded.split_once("--")?;

    if provider.is_empty() || repo_name.is_empty() {
        return None;
    }

    Some(format!("{provider}/{repo_name}"))
}

fn scan_repo_cache(repo_path: &Path, repo_id: &str) -> Result<Option<LocalModel>, String> {
    let refs_main = read_refs_main(repo_path);
    let snapshots = read_snapshot_dirs(&repo_path.join("snapshots"))?;
    let Some(snapshot_path) = choose_snapshot(&snapshots, refs_main.as_deref()) else {
        return Ok(None);
    };
    let files = collect_snapshot_files(&snapshot_path, &snapshot_path)?;
    let primary_file = files
        .iter()
        .find(|file| file.format != ModelFormat::Unknown)
        .or_else(|| files.first());
    let provider = repo_id
        .split_once('/')
        .map(|(provider, _repo)| provider.to_string());
    let display_name = primary_file
        .map(|file| file.name.clone())
        .unwrap_or_else(|| repo_id.rsplit('/').next().unwrap_or(repo_id).to_string());
    let format = primary_file.map(|file| file.format.clone());
    let quantization = primary_file
        .and_then(|file| file.quantization.clone())
        .or_else(|| files.iter().find_map(|file| file.quantization.clone()));
    let size_bytes = calculate_blob_size(&repo_path.join("blobs"))
        .or_else(|| Some(files.iter().filter_map(|file| file.size_bytes).sum::<u64>()))
        .filter(|size| *size > 0);
    let last_modified = latest_modified(&snapshot_path, &snapshot_path)?;

    Ok(Some(LocalModel {
        id: format!("huggingface:{repo_id}"),
        display_name,
        provider,
        repo_id: Some(repo_id.to_string()),
        source: ModelSource::HuggingFace,
        path: Some(path_to_string(&snapshot_path)),
        size_bytes,
        format,
        quantization,
        parameter_size: None,
        last_modified,
        files,
        runtime_status: Some(ModelRuntimeStatus::Available),
    }))
}

fn read_refs_main(repo_path: &Path) -> Option<String> {
    fs::read_to_string(repo_path.join("refs").join("main"))
        .ok()
        .map(|contents| contents.trim().to_string())
        .filter(|contents| !contents.is_empty())
}

fn read_snapshot_dirs(snapshot_root: &Path) -> Result<Vec<PathBuf>, String> {
    if !snapshot_root.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(snapshot_root).map_err(|error| {
        format!(
            "Could not read snapshots folder at {}: {error}",
            snapshot_root.display()
        )
    })?;
    let mut snapshots = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not read snapshot entry: {error}"))?;
        let path = entry.path();

        if path.is_dir() {
            snapshots.push(path);
        }
    }

    Ok(snapshots)
}

fn choose_snapshot(snapshots: &[PathBuf], refs_main: Option<&str>) -> Option<PathBuf> {
    if let Some(refs_main) = refs_main {
        if let Some(snapshot) = snapshots.iter().find(|snapshot| {
            snapshot
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == refs_main)
                .unwrap_or(false)
        }) {
            return Some(snapshot.clone());
        }
    }

    snapshots
        .iter()
        .max_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .cloned()
}

fn collect_snapshot_files(
    snapshot_path: &Path,
    current_path: &Path,
) -> Result<Vec<LocalModelFile>, String> {
    let entries = fs::read_dir(current_path).map_err(|error| {
        format!(
            "Could not read snapshot folder at {}: {error}",
            current_path.display()
        )
    })?;
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not read snapshot file: {error}"))?;
        let path = entry.path();
        let symlink_metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => return Err(format!("Could not inspect {}: {error}", path.display())),
        };

        if symlink_metadata.is_dir() {
            files.extend(collect_snapshot_files(snapshot_path, &path)?);
            continue;
        }

        let metadata = fs::metadata(&path).ok();
        let relative_path = path
            .strip_prefix(snapshot_path)
            .unwrap_or(path.as_path())
            .to_path_buf();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path_to_string(&relative_path));
        let format = detect_format(&path);

        files.push(LocalModelFile {
            name: name.clone(),
            path: path_to_string(&relative_path),
            size_bytes: metadata.map(|metadata| metadata.len()),
            format,
            quantization: parse_quantization_from_name(&name),
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(files)
}

fn calculate_blob_size(blobs_path: &Path) -> Option<u64> {
    let entries = fs::read_dir(blobs_path).ok()?;
    let mut size = 0;

    for entry in entries.flatten() {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() {
                size += metadata.len();
            }
        }
    }

    Some(size)
}

fn latest_modified(snapshot_path: &Path, current_path: &Path) -> Result<Option<String>, String> {
    let entries = fs::read_dir(current_path).map_err(|error| {
        format!(
            "Could not read snapshot folder at {}: {error}",
            current_path.display()
        )
    })?;
    let mut latest: Option<SystemTime> = None;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not read snapshot file: {error}"))?;
        let path = entry.path();
        let metadata = fs::metadata(&path)
            .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;

        if metadata.is_dir() {
            let nested_latest = latest_modified(snapshot_path, &path)?;
            if let Some(nested_latest) =
                nested_latest.and_then(|timestamp| timestamp.parse::<u64>().ok())
            {
                let nested_time = UNIX_EPOCH + std::time::Duration::from_secs(nested_latest);
                latest = Some(match latest {
                    Some(current) => current.max(nested_time),
                    None => nested_time,
                });
            }
        } else if let Ok(modified) = metadata.modified() {
            latest = Some(match latest {
                Some(current) => current.max(modified),
                None => modified,
            });
        }
    }

    let _ = snapshot_path;

    Ok(latest.and_then(system_time_to_timestamp))
}

fn system_time_to_timestamp(time: SystemTime) -> Option<String> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs().to_string())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("hf_cache_sample")
    }

    #[test]
    fn encodes_and_decodes_hf_cache_repo_folders() {
        assert_eq!(
            encode_repo_id("Qwen/Qwen3-4B"),
            Some("models--Qwen--Qwen3-4B".to_string())
        );
        assert_eq!(
            decode_cache_repo_dir_name("models--Qwen--Qwen3-4B"),
            Some("Qwen/Qwen3-4B".to_string())
        );
        assert_eq!(decode_cache_repo_dir_name("datasets--org--repo"), None);
        assert_eq!(encode_repo_id("invalid"), None);
    }

    #[test]
    fn scans_fixture_cache_models_and_uses_blob_size() {
        let report = scan_cache(&fixture_root()).expect("fixture should scan");

        assert_eq!(report.models.len(), 1);
        assert_eq!(report.models[0].repo_id.as_deref(), Some("Qwen/Qwen3-4B"));
        assert_eq!(report.models[0].provider.as_deref(), Some("Qwen"));
        assert_eq!(report.models[0].size_bytes, Some(15));
        assert_eq!(report.models[0].format, Some(ModelFormat::Gguf));
        assert_eq!(report.models[0].quantization.as_deref(), Some("Q4_K_M"));
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("Broken/NoSnapshots")));
    }
}
