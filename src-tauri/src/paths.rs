use std::env;
use std::path::{Component, Path, PathBuf};

use crate::models::{AppSettings, PathIssue, PathResolutionSource, ResolvedPath, ResolvedPaths};

#[derive(Debug, Clone, Default)]
pub struct PathInputs {
    pub user_profile: Option<PathBuf>,
    pub hf_home: Option<PathBuf>,
    pub hf_hub_cache: Option<PathBuf>,
}

impl PathInputs {
    pub fn from_env() -> Self {
        Self {
            user_profile: env_path("USERPROFILE")
                .or_else(|| env_path("HOME"))
                .or_else(home_from_drive_env),
            hf_home: env_path("HF_HOME"),
            hf_hub_cache: env_path("HF_HUB_CACHE"),
        }
    }
}

pub fn resolve_paths(settings: &AppSettings) -> ResolvedPaths {
    resolve_paths_with_inputs(settings, &PathInputs::from_env())
}

pub fn resolve_paths_with_inputs(settings: &AppSettings, inputs: &PathInputs) -> ResolvedPaths {
    let hf_cache = resolve_hf_cache(settings, inputs);
    let lm_studio_models = resolve_lm_studio_models(settings, inputs);
    let custom_model_folders = resolve_custom_model_folders(settings);

    ResolvedPaths {
        hf_cache,
        lm_studio_models,
        custom_model_folders,
    }
}

fn resolve_hf_cache(settings: &AppSettings, inputs: &PathInputs) -> ResolvedPath {
    if let Some(path) = settings.hf_cache_path.as_ref() {
        return build_resolved_path(
            "Hugging Face cache",
            Some(PathBuf::from(path)),
            PathResolutionSource::UserSetting,
            "User setting",
            PathKind::ModelRoot,
        );
    }

    if let Some(path) = inputs.hf_hub_cache.as_ref() {
        return build_resolved_path(
            "Hugging Face cache",
            Some(path.clone()),
            PathResolutionSource::Environment,
            "HF_HUB_CACHE",
            PathKind::ModelRoot,
        );
    }

    if let Some(path) = inputs.hf_home.as_ref() {
        return build_resolved_path(
            "Hugging Face cache",
            Some(path.join("hub")),
            PathResolutionSource::Environment,
            "HF_HOME + hub",
            PathKind::ModelRoot,
        );
    }

    build_resolved_path(
        "Hugging Face cache",
        inputs
            .user_profile
            .as_ref()
            .map(|path| path.join(".cache").join("huggingface").join("hub")),
        PathResolutionSource::Default,
        "Default Windows path",
        PathKind::ModelRoot,
    )
}

fn resolve_lm_studio_models(settings: &AppSettings, inputs: &PathInputs) -> ResolvedPath {
    if let Some(path) = settings.lm_studio_models_path.as_ref() {
        return build_resolved_path(
            "LM Studio models",
            Some(PathBuf::from(path)),
            PathResolutionSource::UserSetting,
            "User setting",
            PathKind::ModelRoot,
        );
    }

    build_resolved_path(
        "LM Studio models",
        inputs
            .user_profile
            .as_ref()
            .map(|path| path.join(".lmstudio").join("models")),
        PathResolutionSource::Default,
        "Default Windows path",
        PathKind::ModelRoot,
    )
}

fn resolve_custom_model_folders(settings: &AppSettings) -> Vec<ResolvedPath> {
    let mut resolved_paths: Vec<ResolvedPath> = settings
        .custom_model_folders
        .iter()
        .enumerate()
        .map(|(index, path)| {
            build_resolved_path(
                &format!("Custom folder {}", index + 1),
                Some(PathBuf::from(path)),
                PathResolutionSource::UserSetting,
                "User setting",
                PathKind::CustomFolder,
            )
        })
        .collect();

    add_overlap_warnings(&mut resolved_paths);
    resolved_paths
}

fn build_resolved_path(
    label: &str,
    path: Option<PathBuf>,
    source: PathResolutionSource,
    source_label: &str,
    kind: PathKind,
) -> ResolvedPath {
    let mut issues = Vec::new();

    let Some(path) = path else {
        return ResolvedPath {
            label: label.to_string(),
            path: None,
            source: PathResolutionSource::Unresolved,
            source_label: "Unresolved".to_string(),
            exists: false,
            is_directory: false,
            issues: vec![PathIssue::error(
                "unresolved_home",
                "ModelHub could not resolve your Windows user profile path.",
            )],
        };
    };

    if !path.is_absolute() {
        issues.push(PathIssue::error(
            "relative_path",
            "Use an absolute Windows path so scanners and downloads have a stable root.",
        ));
    }

    if kind == PathKind::CustomFolder && is_drive_root(&path) {
        issues.push(PathIssue::error(
            "drive_root",
            "Choose a specific model folder instead of an entire drive root.",
        ));
    }

    let exists = path.exists();
    let is_directory = path.is_dir();

    if exists && !is_directory {
        issues.push(PathIssue::error(
            "not_directory",
            "This path exists but is not a folder.",
        ));
    } else if !exists {
        issues.push(PathIssue::warning(
            "missing",
            "This folder does not exist yet. Scans will skip it until it is available.",
        ));
    }

    ResolvedPath {
        label: label.to_string(),
        path: Some(path_to_string(&path)),
        source,
        source_label: source_label.to_string(),
        exists,
        is_directory,
        issues,
    }
}

fn add_overlap_warnings(paths: &mut [ResolvedPath]) {
    let normalized_paths: Vec<Option<String>> = paths
        .iter()
        .map(|path| path.path.as_deref().map(normalize_for_compare))
        .collect();

    for current_index in 0..paths.len() {
        let Some(current) = normalized_paths[current_index].as_ref() else {
            continue;
        };

        let overlaps = normalized_paths
            .iter()
            .enumerate()
            .any(|(other_index, other)| {
                if current_index == other_index {
                    return false;
                }

                other
                    .as_ref()
                    .map(|other| is_nested_path(current, other) || is_nested_path(other, current))
                    .unwrap_or(false)
            });

        if overlaps {
            paths[current_index].issues.push(PathIssue::warning(
                "overlap",
                "This folder overlaps another custom folder and may cause duplicate scan results.",
            ));
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

fn home_from_drive_env() -> Option<PathBuf> {
    let drive = env::var_os("HOMEDRIVE")?;
    let path = env::var_os("HOMEPATH")?;

    Some(PathBuf::from(format!(
        "{}{}",
        drive.to_string_lossy(),
        path.to_string_lossy()
    )))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn normalize_for_compare(path: &str) -> String {
    let normalized = path.replace('/', "\\").to_lowercase();
    let trimmed = normalized.trim_end_matches('\\');

    if trimmed.ends_with(':') {
        format!("{trimmed}\\")
    } else {
        trimmed.to_string()
    }
}

fn is_nested_path(candidate: &str, parent: &str) -> bool {
    candidate.len() > parent.len()
        && candidate.starts_with(parent)
        && candidate[parent.len()..].starts_with('\\')
}

fn is_drive_root(path: &Path) -> bool {
    let mut components = path.components();

    matches!(components.next(), Some(Component::Prefix(_)))
        && matches!(components.next(), Some(Component::RootDir))
        && components.next().is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathKind {
    ModelRoot,
    CustomFolder,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSettings, PathIssueSeverity};

    fn inputs() -> PathInputs {
        PathInputs {
            user_profile: Some(PathBuf::from("C:\\Users\\demo")),
            hf_home: None,
            hf_hub_cache: None,
        }
    }

    #[test]
    fn hf_cache_prefers_user_setting() {
        let settings = AppSettings {
            hf_cache_path: Some("D:\\HF\\hub".to_string()),
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert_eq!(resolved.hf_cache.path.as_deref(), Some("D:\\HF\\hub"));
        assert_eq!(resolved.hf_cache.source, PathResolutionSource::UserSetting);
    }

    #[test]
    fn hf_cache_prefers_hf_hub_cache_over_hf_home() {
        let mut inputs = inputs();
        inputs.hf_hub_cache = Some(PathBuf::from("E:\\hf-hub-cache"));
        inputs.hf_home = Some(PathBuf::from("E:\\hf-home"));

        let resolved = resolve_paths_with_inputs(&AppSettings::default(), &inputs);

        assert_eq!(resolved.hf_cache.path.as_deref(), Some("E:\\hf-hub-cache"));
        assert_eq!(resolved.hf_cache.source_label, "HF_HUB_CACHE");
    }

    #[test]
    fn hf_home_resolves_to_hub_folder() {
        let mut inputs = inputs();
        inputs.hf_home = Some(PathBuf::from("E:\\hf-home"));

        let resolved = resolve_paths_with_inputs(&AppSettings::default(), &inputs);

        assert_eq!(resolved.hf_cache.path.as_deref(), Some("E:\\hf-home\\hub"));
        assert_eq!(resolved.hf_cache.source_label, "HF_HOME + hub");
    }

    #[test]
    fn default_paths_use_user_profile() {
        let resolved = resolve_paths_with_inputs(&AppSettings::default(), &inputs());

        assert_eq!(
            resolved.hf_cache.path.as_deref(),
            Some("C:\\Users\\demo\\.cache\\huggingface\\hub")
        );
        assert_eq!(
            resolved.lm_studio_models.path.as_deref(),
            Some("C:\\Users\\demo\\.lmstudio\\models")
        );
    }

    #[test]
    fn missing_user_profile_returns_unresolved_error() {
        let resolved = resolve_paths_with_inputs(&AppSettings::default(), &PathInputs::default());

        assert_eq!(resolved.hf_cache.path, None);
        assert_eq!(resolved.hf_cache.source, PathResolutionSource::Unresolved);
        assert_eq!(
            resolved.hf_cache.issues[0].severity,
            PathIssueSeverity::Error
        );
    }

    #[test]
    fn relative_paths_are_reported_as_errors() {
        let settings = AppSettings {
            hf_cache_path: Some("models\\hf".to_string()),
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert!(resolved.hf_cache.issues.iter().any(
            |issue| issue.code == "relative_path" && issue.severity == PathIssueSeverity::Error
        ));
    }

    #[test]
    fn missing_paths_are_warnings_not_fatal() {
        let settings = AppSettings {
            lm_studio_models_path: Some("Z:\\probably-missing\\models".to_string()),
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert!(resolved
            .lm_studio_models
            .issues
            .iter()
            .any(|issue| issue.code == "missing" && issue.severity == PathIssueSeverity::Warning));
    }

    #[test]
    fn custom_drive_roots_are_reported_as_errors() {
        let settings = AppSettings {
            custom_model_folders: vec!["D:\\".to_string()],
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert!(resolved.custom_model_folders[0]
            .issues
            .iter()
            .any(|issue| issue.code == "drive_root"));
    }

    #[test]
    fn overlapping_custom_folders_get_warnings() {
        let settings = AppSettings {
            custom_model_folders: vec!["D:\\Models".to_string(), "D:\\Models\\GGUF".to_string()],
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert!(resolved
            .custom_model_folders
            .iter()
            .all(|path| path.issues.iter().any(|issue| issue.code == "overlap")));
    }

    #[test]
    fn existing_directory_paths_are_confirmed() {
        let directory = tempfile::tempdir().expect("temp model dir");
        let settings = AppSettings {
            hf_cache_path: Some(directory.path().to_string_lossy().to_string()),
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert!(resolved.hf_cache.exists);
        assert!(resolved.hf_cache.is_directory);
        assert!(resolved.hf_cache.issues.is_empty());
    }

    #[test]
    fn existing_files_are_not_valid_model_roots() {
        let directory = tempfile::tempdir().expect("temp model dir");
        let file_path = directory.path().join("model.gguf");
        std::fs::write(&file_path, "model").expect("model file should write");
        let settings = AppSettings {
            hf_cache_path: Some(file_path.to_string_lossy().to_string()),
            ..AppSettings::default()
        };

        let resolved = resolve_paths_with_inputs(&settings, &inputs());

        assert!(resolved.hf_cache.exists);
        assert!(!resolved.hf_cache.is_directory);
        assert!(resolved
            .hf_cache
            .issues
            .iter()
            .any(|issue| issue.code == "not_directory"));
    }
}
