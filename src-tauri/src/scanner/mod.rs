pub mod common;
pub mod huggingface;
pub mod lmstudio;
pub mod ollama;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{
    ModelSource, PathIssueSeverity, ResolvedPath, ResolvedPaths, ScanResult, SourceScanStatus,
    SourceStatus,
};

pub fn scan_models(paths: &ResolvedPaths) -> ScanResult {
    scan_models_with_ollama(paths, scan_ollama)
}

fn scan_models_with_ollama(
    paths: &ResolvedPaths,
    scan_ollama_source: impl FnOnce() -> SourceScan,
) -> ScanResult {
    let mut models = Vec::new();
    let mut source_statuses = Vec::new();

    match scan_hugging_face(&paths.hf_cache) {
        SourceScan::Models {
            mut scanned_models,
            status,
        } => {
            models.append(&mut scanned_models);
            source_statuses.push(status);
        }
        SourceScan::Status(status) => source_statuses.push(status),
    }

    match scan_lm_studio(&paths.lm_studio_models) {
        SourceScan::Models {
            mut scanned_models,
            status,
        } => {
            models.append(&mut scanned_models);
            source_statuses.push(status);
        }
        SourceScan::Status(status) => source_statuses.push(status),
    }

    match scan_ollama_source() {
        SourceScan::Models {
            mut scanned_models,
            status,
        } => {
            models.append(&mut scanned_models);
            source_statuses.push(status);
        }
        SourceScan::Status(status) => source_statuses.push(status),
    }

    let total_size_bytes = models
        .iter()
        .filter_map(|model| model.size_bytes)
        .sum::<u64>();

    ScanResult {
        models,
        source_statuses,
        total_size_bytes: if total_size_bytes > 0 {
            Some(total_size_bytes)
        } else {
            None
        },
        scanned_at: timestamp_now(),
    }
}

fn scan_ollama() -> SourceScan {
    match ollama::scan_models() {
        Ok(report) => SourceScan::Models {
            scanned_models: report.models,
            status: SourceStatus {
                source: ModelSource::Ollama,
                status: SourceScanStatus::Ok,
                path: Some(ollama::OLLAMA_BASE_URL.to_string()),
                message: None,
            },
        },
        Err(ollama::OllamaScanError::NotRunning) => SourceScan::Status(SourceStatus {
            source: ModelSource::Ollama,
            status: SourceScanStatus::Disabled,
            path: Some(ollama::OLLAMA_BASE_URL.to_string()),
            message: Some(ollama::OllamaScanError::NotRunning.user_message()),
        }),
        Err(error) => SourceScan::Status(SourceStatus {
            source: ModelSource::Ollama,
            status: SourceScanStatus::Error,
            path: Some(ollama::OLLAMA_BASE_URL.to_string()),
            message: Some(error.user_message()),
        }),
    }
}

fn scan_hugging_face(path: &ResolvedPath) -> SourceScan {
    let issue_errors = path
        .issues
        .iter()
        .filter(|issue| issue.severity == PathIssueSeverity::Error)
        .map(|issue| issue.message.clone())
        .collect::<Vec<_>>();

    if !issue_errors.is_empty() {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::HuggingFace,
            status: SourceScanStatus::Error,
            path: path.path.clone(),
            message: Some(issue_errors.join(" ")),
        });
    }

    let Some(cache_path) = path.path.as_deref() else {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::HuggingFace,
            status: SourceScanStatus::Error,
            path: None,
            message: Some("Hugging Face cache path could not be resolved.".to_string()),
        });
    };

    if !path.exists {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::HuggingFace,
            status: SourceScanStatus::Missing,
            path: Some(cache_path.to_string()),
            message: Some("Hugging Face cache folder does not exist yet.".to_string()),
        });
    }

    if !path.is_directory {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::HuggingFace,
            status: SourceScanStatus::Error,
            path: Some(cache_path.to_string()),
            message: Some("Hugging Face cache path is not a folder.".to_string()),
        });
    }

    match huggingface::scan_cache(cache_path.as_ref()) {
        Ok(report) => {
            let warning_message = if report.warnings.is_empty() {
                None
            } else {
                Some(format!(
                    "{} Hugging Face cache {}: {}",
                    report.warnings.len(),
                    if report.warnings.len() == 1 {
                        "warning"
                    } else {
                        "warnings"
                    },
                    report.warnings.join(" ")
                ))
            };

            SourceScan::Models {
                scanned_models: report.models,
                status: SourceStatus {
                    source: ModelSource::HuggingFace,
                    status: SourceScanStatus::Ok,
                    path: Some(cache_path.to_string()),
                    message: warning_message,
                },
            }
        }
        Err(message) => SourceScan::Status(SourceStatus {
            source: ModelSource::HuggingFace,
            status: SourceScanStatus::Error,
            path: Some(cache_path.to_string()),
            message: Some(message),
        }),
    }
}

fn scan_lm_studio(path: &ResolvedPath) -> SourceScan {
    let issue_errors = path
        .issues
        .iter()
        .filter(|issue| issue.severity == PathIssueSeverity::Error)
        .map(|issue| issue.message.clone())
        .collect::<Vec<_>>();

    if !issue_errors.is_empty() {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::LmStudio,
            status: SourceScanStatus::Error,
            path: path.path.clone(),
            message: Some(issue_errors.join(" ")),
        });
    }

    let Some(models_path) = path.path.as_deref() else {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::LmStudio,
            status: SourceScanStatus::Error,
            path: None,
            message: Some("LM Studio models path could not be resolved.".to_string()),
        });
    };

    if !path.exists {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::LmStudio,
            status: SourceScanStatus::Missing,
            path: Some(models_path.to_string()),
            message: Some("LM Studio models folder does not exist yet.".to_string()),
        });
    }

    if !path.is_directory {
        return SourceScan::Status(SourceStatus {
            source: ModelSource::LmStudio,
            status: SourceScanStatus::Error,
            path: Some(models_path.to_string()),
            message: Some("LM Studio models path is not a folder.".to_string()),
        });
    }

    match lmstudio::scan_models(models_path.as_ref()) {
        Ok(report) => {
            let warning_message = if report.warnings.is_empty() {
                None
            } else {
                Some(format!(
                    "{} LM Studio scan {}: {}",
                    report.warnings.len(),
                    if report.warnings.len() == 1 {
                        "warning"
                    } else {
                        "warnings"
                    },
                    report.warnings.join(" ")
                ))
            };

            SourceScan::Models {
                scanned_models: report.models,
                status: SourceStatus {
                    source: ModelSource::LmStudio,
                    status: SourceScanStatus::Ok,
                    path: Some(models_path.to_string()),
                    message: warning_message,
                },
            }
        }
        Err(message) => SourceScan::Status(SourceStatus {
            source: ModelSource::LmStudio,
            status: SourceScanStatus::Error,
            path: Some(models_path.to_string()),
            message: Some(message),
        }),
    }
}

enum SourceScan {
    Models {
        scanned_models: Vec<crate::models::LocalModel>,
        status: SourceStatus,
    },
    Status(SourceStatus),
}

fn timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::models::{
        LocalModel, ModelFormat, ModelRuntimeStatus, PathResolutionSource, ResolvedPath,
        ResolvedPaths,
    };

    #[test]
    fn scan_models_combines_hf_lm_studio_and_ollama_sources() {
        let result = scan_models_with_ollama(
            &ResolvedPaths {
                hf_cache: resolved_fixture_path(
                    &fixture_path("hf_cache_sample"),
                    "Hugging Face cache",
                ),
                lm_studio_models: resolved_fixture_path(
                    &fixture_path("lmstudio_sample"),
                    "LM Studio models",
                ),
                custom_model_folders: Vec::new(),
            },
            ollama_model_scan,
        );

        assert_eq!(result.source_statuses.len(), 3);
        assert_eq!(
            result
                .source_statuses
                .iter()
                .filter(|status| status.source == ModelSource::HuggingFace)
                .count(),
            1
        );
        assert_eq!(
            result
                .source_statuses
                .iter()
                .filter(|status| status.source == ModelSource::LmStudio)
                .count(),
            1
        );
        assert_eq!(
            result
                .source_statuses
                .iter()
                .filter(|status| status.source == ModelSource::Ollama)
                .count(),
            1
        );
        assert!(result.source_statuses.iter().any(|status| {
            status.source == ModelSource::HuggingFace && status.status == SourceScanStatus::Ok
        }));
        assert!(result.source_statuses.iter().any(|status| {
            status.source == ModelSource::LmStudio && status.status == SourceScanStatus::Ok
        }));
        assert!(result.source_statuses.iter().any(|status| {
            status.source == ModelSource::Ollama && status.status == SourceScanStatus::Ok
        }));
        assert_eq!(result.models.len(), 4);
        assert_eq!(
            result
                .models
                .iter()
                .filter(|model| model.source == ModelSource::HuggingFace)
                .count(),
            1
        );
        assert_eq!(
            result
                .models
                .iter()
                .filter(|model| model.source == ModelSource::LmStudio)
                .count(),
            2
        );
        assert_eq!(
            result
                .models
                .iter()
                .filter(|model| model.source == ModelSource::Ollama)
                .count(),
            1
        );
        let unique_ids = result
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(unique_ids.len(), result.models.len());
        assert_eq!(
            result.total_size_bytes,
            Some(
                result
                    .models
                    .iter()
                    .filter_map(|model| model.size_bytes)
                    .sum()
            )
        );
    }

    #[test]
    fn missing_lm_studio_folder_is_non_fatal() {
        let result = scan_models_with_ollama(
            &ResolvedPaths {
                hf_cache: resolved_fixture_path(
                    &fixture_path("hf_cache_sample"),
                    "Hugging Face cache",
                ),
                lm_studio_models: ResolvedPath {
                    label: "LM Studio models".to_string(),
                    path: Some(path_to_string(&fixture_path("missing_lmstudio"))),
                    source: PathResolutionSource::Default,
                    source_label: "Default Windows path".to_string(),
                    exists: false,
                    is_directory: false,
                    issues: Vec::new(),
                },
                custom_model_folders: Vec::new(),
            },
            ollama_disabled_scan,
        );

        assert!(result
            .source_statuses
            .iter()
            .any(|status| status.source == ModelSource::LmStudio
                && status.status == SourceScanStatus::Missing));
        assert!(result
            .models
            .iter()
            .any(|model| model.source == ModelSource::HuggingFace));
    }

    #[test]
    fn lm_studio_warnings_are_reported_as_source_messages() {
        let directory = tempfile::tempdir().expect("temp lmstudio dir");
        let model_dir = directory.path().join("publisher").join("model");
        fs::create_dir_all(&model_dir).expect("model dir should create");
        fs::write(model_dir.join("model-Q4_K_M.gguf"), "").expect("zero-byte file should write");

        let result = scan_models_with_ollama(
            &ResolvedPaths {
                hf_cache: resolved_fixture_path(
                    &fixture_path("hf_cache_sample"),
                    "Hugging Face cache",
                ),
                lm_studio_models: resolved_fixture_path(directory.path(), "LM Studio models"),
                custom_model_folders: Vec::new(),
            },
            ollama_disabled_scan,
        );
        let lm_studio_status = result
            .source_statuses
            .iter()
            .find(|status| status.source == ModelSource::LmStudio)
            .expect("LM Studio source status should exist");

        assert_eq!(lm_studio_status.status, SourceScanStatus::Ok);
        assert!(lm_studio_status
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("1 LM Studio scan warning"));
        assert!(lm_studio_status
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("zero-byte"));
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn resolved_fixture_path(path: &Path, label: &str) -> ResolvedPath {
        ResolvedPath {
            label: label.to_string(),
            path: Some(path_to_string(path)),
            source: PathResolutionSource::UserSetting,
            source_label: "Test fixture".to_string(),
            exists: true,
            is_directory: true,
            issues: Vec::new(),
        }
    }

    fn path_to_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    fn ollama_model_scan() -> SourceScan {
        SourceScan::Models {
            scanned_models: vec![LocalModel {
                id: "ollama:llama3.2:latest".to_string(),
                display_name: "llama3.2:latest".to_string(),
                provider: None,
                repo_id: Some("llama3.2:latest".to_string()),
                source: ModelSource::Ollama,
                path: None,
                size_bytes: Some(42),
                format: Some(ModelFormat::Gguf),
                quantization: Some("Q4_K_M".to_string()),
                parameter_size: Some("3.2B".to_string()),
                last_modified: Some("2024-09-25T19:22:00Z".to_string()),
                files: Vec::new(),
                runtime_status: Some(ModelRuntimeStatus::Available),
            }],
            status: SourceStatus {
                source: ModelSource::Ollama,
                status: SourceScanStatus::Ok,
                path: Some(ollama::OLLAMA_BASE_URL.to_string()),
                message: None,
            },
        }
    }

    fn ollama_disabled_scan() -> SourceScan {
        SourceScan::Status(SourceStatus {
            source: ModelSource::Ollama,
            status: SourceScanStatus::Disabled,
            path: Some(ollama::OLLAMA_BASE_URL.to_string()),
            message: Some("Ollama is not running on localhost:11434.".to_string()),
        })
    }
}
