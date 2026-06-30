use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::models::{DeleteModelInput, DeleteResult, ModelSource, ResolvedPath, ResolvedPaths};

pub fn open_path(path: &str) -> Result<(), String> {
    let path = validate_existing_absolute_path(path)?;
    let open_target = if path.is_file() {
        path.parent()
            .ok_or_else(|| "ModelHub could not resolve the file's parent folder.".to_string())?
            .to_path_buf()
    } else {
        path
    };

    open_in_file_manager(&open_target)
}

pub fn delete_model(
    input: DeleteModelInput,
    paths: &ResolvedPaths,
) -> Result<DeleteResult, String> {
    if input.source == ModelSource::Ollama {
        return Err(
            "Ollama models do not expose a local model folder for safe deletion.".to_string(),
        );
    }

    let raw_path = input
        .path
        .as_deref()
        .ok_or_else(|| "This model does not have a local path to delete.".to_string())?;
    let target = validate_delete_target(raw_path, &input.source, paths)?;
    let deleted_path = path_to_string(&target);

    trash::delete(&target).map_err(|error| {
        format!(
            "ModelHub could not move {} to the Recycle Bin: {error}",
            target.display()
        )
    })?;

    Ok(DeleteResult {
        deleted_path,
        used_recycle_bin: true,
        message: "Model moved to the Recycle Bin.".to_string(),
    })
}

fn validate_existing_absolute_path(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.trim());

    if !path.is_absolute() || has_parent_component(&path) {
        return Err("ModelHub rejected an unsafe path.".to_string());
    }

    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    Ok(path)
}

fn validate_delete_target(
    path: &str,
    source: &ModelSource,
    paths: &ResolvedPaths,
) -> Result<PathBuf, String> {
    let target = validate_existing_absolute_path(path)?;
    let canonical_target = canonicalize_existing(&target)?;

    match source {
        ModelSource::HuggingFace => validate_hf_snapshot_target(&canonical_target, &paths.hf_cache),
        ModelSource::LmStudio => {
            validate_under_root(&canonical_target, &paths.lm_studio_models, "LM Studio")
        }
        ModelSource::Custom => {
            validate_under_any_custom_root(&canonical_target, &paths.custom_model_folders)
        }
        ModelSource::Ollama => {
            Err("Ollama models do not expose a local model folder for safe deletion.".to_string())
        }
    }
}

fn validate_hf_snapshot_target(target: &Path, hf_cache: &ResolvedPath) -> Result<PathBuf, String> {
    let root = canonical_root(hf_cache, "Hugging Face cache")?;

    if target == root {
        return Err("ModelHub will not delete the entire Hugging Face cache root.".to_string());
    }

    if !is_descendant(target, &root) {
        return Err("Model path is outside the configured Hugging Face cache.".to_string());
    }

    let relative = target
        .strip_prefix(&root)
        .map_err(|_| "ModelHub could not verify this Hugging Face cache path.".to_string())?;
    let parts = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();

    if parts.len() == 3
        && parts[0].starts_with("models--")
        && parts[1] == "snapshots"
        && !parts[2].trim().is_empty()
        && target.is_dir()
    {
        Ok(target.to_path_buf())
    } else {
        Err("For Hugging Face models, ModelHub only deletes snapshot folders, not shared blobs or cache roots.".to_string())
    }
}

fn validate_under_root(target: &Path, root: &ResolvedPath, label: &str) -> Result<PathBuf, String> {
    let root = canonical_root(root, label)?;

    if target == root {
        return Err(format!("ModelHub will not delete the {label} root folder."));
    }

    if !is_descendant(target, &root) {
        return Err(format!(
            "Model path is outside the configured {label} root."
        ));
    }

    Ok(target.to_path_buf())
}

fn validate_under_any_custom_root(
    target: &Path,
    roots: &[ResolvedPath],
) -> Result<PathBuf, String> {
    for root in roots {
        if let Ok(canonical_root) = canonical_root(root, &root.label) {
            if target == canonical_root {
                return Err("ModelHub will not delete a configured custom root folder.".to_string());
            }

            if is_descendant(target, &canonical_root) {
                return Ok(target.to_path_buf());
            }
        }
    }

    Err("Model path is outside configured custom model folders.".to_string())
}

fn canonical_root(path: &ResolvedPath, label: &str) -> Result<PathBuf, String> {
    let raw_path = path
        .path
        .as_deref()
        .ok_or_else(|| format!("{label} path is not configured."))?;
    let root = validate_existing_absolute_path(raw_path)?;

    if !root.is_dir() {
        return Err(format!("{label} path is not a folder."));
    }

    canonicalize_existing(&root)
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path)
        .map_err(|error| format!("ModelHub could not inspect {}: {error}", path.display()))
}

fn is_descendant(path: &Path, root: &Path) -> bool {
    path.starts_with(root) && path != root
}

fn has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(windows)]
fn open_in_file_manager(path: &Path) -> Result<(), String> {
    Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("ModelHub could not open {}: {error}", path.display()))
}

#[cfg(target_os = "macos")]
fn open_in_file_manager(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("ModelHub could not open {}: {error}", path.display()))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_file_manager(path: &Path) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("ModelHub could not open {}: {error}", path.display()))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PathResolutionSource, ResolvedPath};

    fn resolved_root(label: &str, path: &Path) -> ResolvedPath {
        ResolvedPath {
            label: label.to_string(),
            path: Some(path_to_string(path)),
            source: PathResolutionSource::UserSetting,
            source_label: "Test".to_string(),
            exists: true,
            is_directory: true,
            issues: Vec::new(),
        }
    }

    fn paths(hf_root: &Path, lm_root: &Path, custom_root: &Path) -> ResolvedPaths {
        ResolvedPaths {
            hf_cache: resolved_root("Hugging Face cache", hf_root),
            lm_studio_models: resolved_root("LM Studio models", lm_root),
            custom_model_folders: vec![resolved_root("Custom folder 1", custom_root)],
        }
    }

    #[test]
    fn rejects_delete_outside_known_roots() {
        let directory = tempfile::tempdir().expect("temp dir");
        let hf_root = directory.path().join("hf");
        let lm_root = directory.path().join("lm");
        let custom_root = directory.path().join("custom");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&hf_root).unwrap();
        fs::create_dir_all(&lm_root).unwrap();
        fs::create_dir_all(&custom_root).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let error = validate_delete_target(
            &path_to_string(&outside),
            &ModelSource::LmStudio,
            &paths(&hf_root, &lm_root, &custom_root),
        )
        .expect_err("outside path should fail");

        assert!(error.contains("outside"));
    }

    #[test]
    fn rejects_root_folder_deletion() {
        let directory = tempfile::tempdir().expect("temp dir");
        let hf_root = directory.path().join("hf");
        let lm_root = directory.path().join("lm");
        let custom_root = directory.path().join("custom");
        fs::create_dir_all(&hf_root).unwrap();
        fs::create_dir_all(&lm_root).unwrap();
        fs::create_dir_all(&custom_root).unwrap();

        let error = validate_delete_target(
            &path_to_string(&lm_root),
            &ModelSource::LmStudio,
            &paths(&hf_root, &lm_root, &custom_root),
        )
        .expect_err("root deletion should fail");

        assert!(error.contains("root folder"));
    }

    #[test]
    fn allows_lm_studio_model_folder_under_root() {
        let directory = tempfile::tempdir().expect("temp dir");
        let hf_root = directory.path().join("hf");
        let lm_root = directory.path().join("lm");
        let custom_root = directory.path().join("custom");
        let model_path = lm_root.join("publisher").join("model");
        fs::create_dir_all(&hf_root).unwrap();
        fs::create_dir_all(&model_path).unwrap();
        fs::create_dir_all(&custom_root).unwrap();

        let validated = validate_delete_target(
            &path_to_string(&model_path),
            &ModelSource::LmStudio,
            &paths(&hf_root, &lm_root, &custom_root),
        )
        .expect("model path should validate");

        assert_eq!(validated, fs::canonicalize(model_path).unwrap());
    }

    #[test]
    fn allows_hf_snapshot_but_rejects_blobs_and_repo_root() {
        let directory = tempfile::tempdir().expect("temp dir");
        let hf_root = directory.path().join("hf");
        let lm_root = directory.path().join("lm");
        let custom_root = directory.path().join("custom");
        let repo_root = hf_root.join("models--Qwen--Qwen3-4B");
        let snapshot = repo_root.join("snapshots").join("abc123");
        let blob = repo_root.join("blobs").join("blob-a");
        fs::create_dir_all(&snapshot).unwrap();
        fs::create_dir_all(blob.parent().unwrap()).unwrap();
        fs::write(&blob, b"hello").unwrap();
        fs::create_dir_all(&lm_root).unwrap();
        fs::create_dir_all(&custom_root).unwrap();
        let paths = paths(&hf_root, &lm_root, &custom_root);

        assert!(validate_delete_target(
            &path_to_string(&snapshot),
            &ModelSource::HuggingFace,
            &paths,
        )
        .is_ok());
        assert!(
            validate_delete_target(&path_to_string(&blob), &ModelSource::HuggingFace, &paths,)
                .is_err()
        );
        assert!(validate_delete_target(
            &path_to_string(&repo_root),
            &ModelSource::HuggingFace,
            &paths,
        )
        .is_err());
    }

    #[test]
    fn rejects_ollama_deletion() {
        let directory = tempfile::tempdir().expect("temp dir");
        let hf_root = directory.path().join("hf");
        let lm_root = directory.path().join("lm");
        let custom_root = directory.path().join("custom");
        fs::create_dir_all(&hf_root).unwrap();
        fs::create_dir_all(&lm_root).unwrap();
        fs::create_dir_all(&custom_root).unwrap();

        let input = DeleteModelInput {
            id: "ollama:model".to_string(),
            source: ModelSource::Ollama,
            path: None,
            repo_id: Some("model".to_string()),
        };

        let error = delete_model(input, &paths(&hf_root, &lm_root, &custom_root))
            .expect_err("ollama delete should fail");

        assert!(error.contains("Ollama"));
    }
}
