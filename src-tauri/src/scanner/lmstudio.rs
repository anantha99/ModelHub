use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{LocalModel, LocalModelFile, ModelFormat, ModelRuntimeStatus, ModelSource};

use super::common::{detect_format, parse_quantization_from_name};

pub struct LmStudioScanReport {
    pub models: Vec<LocalModel>,
    pub warnings: Vec<String>,
}

pub fn scan_models(models_root: &Path) -> Result<LmStudioScanReport, String> {
    let publishers = fs::read_dir(models_root).map_err(|error| {
        format!(
            "Could not read LM Studio models folder at {}: {error}",
            models_root.display()
        )
    })?;
    let mut models = Vec::new();
    let mut warnings = Vec::new();

    for publisher_entry in publishers {
        let publisher_entry = match publisher_entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!(
                    "Could not read an LM Studio publisher entry: {error}."
                ));
                continue;
            }
        };
        let publisher_path = publisher_entry.path();

        if !publisher_path.is_dir() {
            continue;
        }

        let publisher = publisher_entry.file_name().to_string_lossy().to_string();

        scan_publisher_dir(&publisher_path, &publisher, &mut models, &mut warnings);
    }

    models.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(LmStudioScanReport { models, warnings })
}

fn scan_publisher_dir(
    publisher_path: &Path,
    publisher: &str,
    models: &mut Vec<LocalModel>,
    warnings: &mut Vec<String>,
) {
    let model_entries = match fs::read_dir(publisher_path) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!(
                "Could not read LM Studio publisher folder {}: {error}.",
                publisher_path.display()
            ));
            return;
        }
    };

    for model_entry in model_entries {
        let model_entry = match model_entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!(
                    "Could not read an LM Studio model entry for {publisher}: {error}."
                ));
                continue;
            }
        };
        let model_path = model_entry.path();

        if !model_path.is_dir() {
            continue;
        }

        let model_name = model_entry.file_name().to_string_lossy().to_string();

        match scan_model_dir(&model_path, publisher, &model_name) {
            Ok(scan) => {
                warnings.extend(scan.warnings);

                if let Some(model) = scan.model {
                    models.push(model);
                }
            }
            Err(error) => warnings.push(format!("{publisher}/{model_name}: {error}")),
        }
    }
}

fn scan_model_dir(
    model_path: &Path,
    publisher: &str,
    model_name: &str,
) -> Result<ModelDirScan, String> {
    let entries = fs::read_dir(model_path).map_err(|error| {
        format!(
            "Could not read LM Studio model folder at {}: {error}",
            model_path.display()
        )
    })?;
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    let mut last_modified: Option<SystemTime> = None;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!("Could not read a model file entry: {error}."));
                continue;
            }
        };
        let path = entry.path();

        if !path.is_file() || detect_format(&path) != ModelFormat::Gguf {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!("Could not inspect {}: {error}.", path.display()));
                continue;
            }
        };

        if metadata.len() == 0 {
            warnings.push(format!("Skipped zero-byte GGUF file {}.", path.display()));
            continue;
        }

        if let Ok(modified) = metadata.modified() {
            last_modified = Some(match last_modified {
                Some(current) => current.max(modified),
                None => modified,
            });
        }

        files.push(local_model_file(model_path, &path, metadata.len()));
    }

    if files.is_empty() {
        return Ok(ModelDirScan {
            model: None,
            warnings,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    let size_bytes = files.iter().filter_map(|file| file.size_bytes).sum::<u64>();
    let quantization = if files.len() == 1 {
        files[0].quantization.clone()
    } else {
        None
    };
    let repo_id = format!("{publisher}/{model_name}");

    Ok(ModelDirScan {
        model: Some(LocalModel {
            id: format!("lmstudio:{repo_id}"),
            display_name: model_name.to_string(),
            provider: Some(publisher.to_string()),
            repo_id: Some(repo_id),
            source: ModelSource::LmStudio,
            path: Some(path_to_string(model_path)),
            size_bytes: Some(size_bytes).filter(|size| *size > 0),
            format: Some(ModelFormat::Gguf),
            quantization,
            parameter_size: None,
            last_modified: last_modified.and_then(system_time_to_timestamp),
            files,
            runtime_status: Some(ModelRuntimeStatus::Available),
        }),
        warnings,
    })
}

struct ModelDirScan {
    model: Option<LocalModel>,
    warnings: Vec<String>,
}

fn local_model_file(model_path: &Path, file_path: &Path, size_bytes: u64) -> LocalModelFile {
    let relative_path = file_path
        .strip_prefix(model_path)
        .unwrap_or(file_path)
        .to_path_buf();
    let name = file_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path_to_string(&relative_path));

    LocalModelFile {
        name: name.clone(),
        path: path_to_string(&relative_path),
        size_bytes: Some(size_bytes),
        format: ModelFormat::Gguf,
        quantization: parse_quantization_from_name(&name),
    }
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
    use std::fs;
    use std::path::{Path, PathBuf};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("lmstudio_sample")
    }

    #[test]
    fn scans_publisher_model_gguf_files() {
        let report = scan_models(&fixture_root()).expect("fixture should scan");

        assert_eq!(report.models.len(), 2);
        let qwen = report
            .models
            .iter()
            .find(|model| model.repo_id.as_deref() == Some("lmstudio-community/Qwen3-4B-GGUF"))
            .expect("qwen model should scan");

        assert_eq!(qwen.source, ModelSource::LmStudio);
        assert_eq!(qwen.provider.as_deref(), Some("lmstudio-community"));
        assert_eq!(qwen.display_name, "Qwen3-4B-GGUF");
        assert!(qwen
            .path
            .as_deref()
            .unwrap_or_default()
            .ends_with("Qwen3-4B-GGUF"));
        assert_eq!(qwen.format, Some(ModelFormat::Gguf));
        assert_eq!(qwen.quantization, None);
        assert_eq!(qwen.files.len(), 2);
        assert!(qwen
            .files
            .iter()
            .all(|file| file.format == ModelFormat::Gguf));
        assert!(qwen
            .files
            .iter()
            .all(|file| !Path::new(&file.path).is_absolute()));
        assert!(
            qwen.files
                .iter()
                .any(|file| file.name == "Qwen3-4B-Q4_K_M.gguf"
                    && file.path == "Qwen3-4B-Q4_K_M.gguf")
        );
        assert!(qwen
            .files
            .iter()
            .any(|file| file.name == "Qwen3-4B-Q8_0.gguf" && file.path == "Qwen3-4B-Q8_0.gguf"));
        assert!(qwen
            .files
            .iter()
            .any(|file| file.quantization.as_deref() == Some("Q4_K_M")));
        assert!(qwen
            .files
            .iter()
            .any(|file| file.quantization.as_deref() == Some("Q8_0")));
        assert_eq!(
            qwen.size_bytes,
            Some(
                fixture_file_size("lmstudio-community/Qwen3-4B-GGUF/Qwen3-4B-Q4_K_M.gguf")
                    + fixture_file_size("lmstudio-community/Qwen3-4B-GGUF/Qwen3-4B-Q8_0.gguf")
            )
        );
    }

    #[test]
    fn parses_iq_quantization_and_ignores_non_gguf_files() {
        let report = scan_models(&fixture_root()).expect("fixture should scan");
        let mistral = report
            .models
            .iter()
            .find(|model| model.repo_id.as_deref() == Some("mistralai/Mistral-Nemo-Instruct"))
            .expect("mistral model should scan");

        assert_eq!(mistral.files.len(), 1);
        assert_eq!(mistral.quantization.as_deref(), Some("IQ4_XS"));
        assert_eq!(mistral.files[0].quantization.as_deref(), Some("IQ4_XS"));
        assert!(!mistral.files.iter().any(|file| file.name == "README.txt"));
    }

    #[test]
    fn existing_empty_root_returns_no_models_without_warning() {
        let directory = tempfile::tempdir().expect("temp lmstudio dir");

        let report = scan_models(directory.path()).expect("empty dir should scan");

        assert!(report.models.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn skips_zero_byte_ggufs_with_warning() {
        let directory = tempfile::tempdir().expect("temp lmstudio dir");
        let model_dir = directory.path().join("publisher").join("model");
        fs::create_dir_all(&model_dir).expect("model dir should create");
        fs::write(model_dir.join("model-Q4_K_M.gguf"), "").expect("zero-byte file should write");

        let report = scan_models(directory.path()).expect("zero-byte dir should scan");

        assert!(report.models.is_empty());
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("zero-byte")));
    }

    fn fixture_file_size(relative_path: &str) -> u64 {
        fs::metadata(fixture_root().join(relative_path))
            .expect("fixture file should exist")
            .len()
    }
}
