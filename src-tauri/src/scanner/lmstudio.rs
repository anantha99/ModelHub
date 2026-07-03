use std::fs;
use std::path::Path;
#[cfg(not(test))]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(test))]
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::models::{LocalModel, LocalModelFile, ModelFormat, ModelRuntimeStatus, ModelSource};

use super::common::{detect_format, parse_quantization_from_name};
use super::metadata::{read_gguf_metadata, LocalModelMetadata};

pub const LM_STUDIO_BASE_URL: &str = "http://localhost:1234";
#[cfg(not(test))]
const LM_STUDIO_TIMEOUT: Duration = Duration::from_millis(500);

pub struct LmStudioScanReport {
    pub models: Vec<LocalModel>,
    pub warnings: Vec<String>,
}

pub fn scan_models(models_root: &Path) -> Result<LmStudioScanReport, String> {
    let mut report = scan_models_from_files(models_root)?;

    enrich_models_from_runtime_api(&mut report.models);

    Ok(report)
}

fn scan_models_from_files(models_root: &Path) -> Result<LmStudioScanReport, String> {
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

#[cfg(not(test))]
fn enrich_models_from_runtime_api(models: &mut [LocalModel]) {
    let Ok(api_models) = fetch_runtime_api_models() else {
        return;
    };

    enrich_models_from_api(models, &api_models);
}

#[cfg(test)]
fn enrich_models_from_runtime_api(_models: &mut [LocalModel]) {
    let _ = LM_STUDIO_BASE_URL;
}

#[cfg(not(test))]
fn fetch_runtime_api_models() -> Result<Vec<LmStudioApiModel>, String> {
    let client = Client::builder()
        .timeout(LM_STUDIO_TIMEOUT)
        .build()
        .map_err(|error| format!("Could not create LM Studio API client: {error}"))?;
    let response = client
        .get(format!("{LM_STUDIO_BASE_URL}/api/v1/models"))
        .send()
        .map_err(|error| format!("Could not check LM Studio API: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "LM Studio API returned HTTP status {}.",
            response.status().as_u16()
        ));
    }

    let body = response
        .text()
        .map_err(|error| format!("Could not read LM Studio API response: {error}"))?;

    parse_api_models_response(&body).map(LmStudioApiModelsResponse::into_models)
}

fn parse_api_models_response(body: &str) -> Result<LmStudioApiModelsResponse, String> {
    serde_json::from_str::<LmStudioApiModelsResponse>(body)
        .map_err(|error| format!("Could not decode LM Studio API models: {error}"))
}

fn enrich_models_from_api(models: &mut [LocalModel], api_models: &[LmStudioApiModel]) {
    for model in models {
        let Some(api_model) = api_models
            .iter()
            .find(|api_model| api_model_matches_local_model(api_model, model))
        else {
            continue;
        };

        enrich_model_from_api(model, api_model);
    }
}

fn enrich_model_from_api(model: &mut LocalModel, api_model: &LmStudioApiModel) {
    let mut changed = false;

    changed |= set_option_if_missing(
        &mut model.provider,
        clean_string(api_model.publisher.clone()),
    );
    changed |= set_option_if_missing(
        &mut model.technical.architecture,
        clean_string(api_model.architecture.clone()),
    );
    changed |= set_option_if_missing(
        &mut model.technical.family,
        clean_string(api_model.architecture.clone()),
    );

    if let Some(architecture) = clean_string(api_model.architecture.clone()) {
        changed |= push_unique_string(&mut model.technical.families, architecture);
    }

    changed |= set_option_if_missing(
        &mut model.quantization,
        api_model
            .quantization
            .as_ref()
            .and_then(quantization_string),
    );

    let parameter_size = api_model.parameter_size.as_ref().and_then(value_string);
    changed |= set_option_if_missing(&mut model.parameter_size, parameter_size.clone());
    changed |= set_option_if_missing(&mut model.technical.parameter_size, parameter_size);
    changed |= set_option_if_missing(
        &mut model.technical.parameter_count,
        api_model.parameter_count.as_ref().and_then(value_u64),
    );

    let context_length = api_model.max_context_length.as_ref().and_then(value_u64);
    changed |= set_option_if_missing(&mut model.technical.max_context_length, context_length);
    changed |= set_option_if_missing(&mut model.technical.context_length, context_length);
    changed |= set_option_if_missing(
        &mut model.size_bytes,
        api_model.size_bytes.as_ref().and_then(value_u64),
    );

    if let Some(format) = api_model.format.as_deref().map(model_format_from_api) {
        if format != ModelFormat::Unknown
            && (model.format.is_none() || model.format == Some(ModelFormat::Unknown))
        {
            model.format = Some(format);
            changed = true;
        }
    }

    if let Some(model_type) = clean_string(api_model.model_type.clone()) {
        if model_type.to_ascii_lowercase().contains("embedding") {
            changed |= set_option_if_missing(&mut model.capabilities.embedding, Some(true));
        }
    }

    if let Some(capabilities) = api_model.capabilities.as_ref() {
        changed |= merge_capabilities(model, capabilities);
    }

    let loaded_instance_status = if api_model.loaded_instances.is_empty() {
        None
    } else {
        Some(ModelRuntimeStatus::Loaded)
    };
    let runtime_status = api_model
        .state
        .as_deref()
        .and_then(runtime_status_from_api)
        .or(loaded_instance_status);

    if let Some(runtime_status) = runtime_status {
        model.runtime_status = Some(runtime_status);
        changed = true;
    }

    if changed {
        push_unique_string(&mut model.metadata_sources, "lmstudio_api".to_string());
    }
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
    let mut model_metadata = LocalModelMetadata::default();

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

        if let Ok(gguf_metadata) = read_gguf_metadata(&path) {
            model_metadata.merge(gguf_metadata);
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
    let parameter_size = model_metadata.technical.parameter_size.clone();

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
            parameter_size,
            last_modified: last_modified.and_then(system_time_to_timestamp),
            files,
            runtime_status: Some(ModelRuntimeStatus::Available),
            technical: model_metadata.technical,
            capabilities: model_metadata.capabilities,
            provenance: model_metadata.provenance,
            metadata_sources: model_metadata.sources,
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

fn api_model_matches_local_model(api_model: &LmStudioApiModel, model: &LocalModel) -> bool {
    let mut candidates = vec![model.display_name.as_str(), model.id.as_str()];

    if let Some(repo_id) = model.repo_id.as_deref() {
        candidates.push(repo_id);
    }

    for file in &model.files {
        candidates.push(file.name.as_str());
        candidates.push(file.path.as_str());
    }

    api_model_keys(api_model).into_iter().any(|api_id| {
        let api_id = normalize_model_key(&api_id);

        candidates.iter().any(|candidate| {
            let candidate = normalize_model_key(candidate);
            api_id == candidate || api_id.ends_with(&format!("/{candidate}"))
        })
    })
}

fn model_format_from_api(format: &str) -> ModelFormat {
    match format.trim().to_ascii_lowercase().as_str() {
        "gguf" => ModelFormat::Gguf,
        "safetensors" => ModelFormat::Safetensors,
        "onnx" => ModelFormat::Onnx,
        "mlx" => ModelFormat::Mlx,
        _ => ModelFormat::Unknown,
    }
}

fn runtime_status_from_api(state: &str) -> Option<ModelRuntimeStatus> {
    match state.trim().to_ascii_lowercase().as_str() {
        "loaded" => Some(ModelRuntimeStatus::Loaded),
        "running" => Some(ModelRuntimeStatus::Running),
        "available" | "not-loaded" | "not_loaded" | "unloaded" => {
            Some(ModelRuntimeStatus::Available)
        }
        _ => None,
    }
}

fn merge_capabilities(model: &mut LocalModel, capabilities: &Value) -> bool {
    let mut changed = false;
    let normalized = capability_names(capabilities);

    if normalized.iter().any(|capability| capability == "vision") {
        changed |= set_option_if_missing(&mut model.capabilities.vision, Some(true));
    }

    if normalized
        .iter()
        .any(|capability| capability == "embedding" || capability == "embeddings")
    {
        changed |= set_option_if_missing(&mut model.capabilities.embedding, Some(true));
    }

    if normalized
        .iter()
        .any(|capability| matches!(capability.as_str(), "tool" | "tools" | "tool_use"))
    {
        changed |= set_option_if_missing(&mut model.capabilities.tool_use, Some(true));
    }

    if normalized
        .iter()
        .any(|capability| matches!(capability.as_str(), "reasoning" | "thinking"))
    {
        changed |= set_option_if_missing(&mut model.capabilities.reasoning, Some(true));
    }

    changed
}

fn set_option_if_missing<T>(target: &mut Option<T>, value: Option<T>) -> bool {
    if target.is_none() && value.is_some() {
        *target = value;
        true
    } else {
        false
    }
}

fn push_unique_string(target: &mut Vec<String>, value: String) -> bool {
    if value.trim().is_empty() || target.iter().any(|existing| existing == &value) {
        return false;
    }

    target.push(value);
    true
}

fn clean_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn value_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value_u64(value).map(|value| value.to_string()))
        .filter(|value| !value.trim().is_empty())
}

fn quantization_string(value: &Value) -> Option<String> {
    value_string(value).or_else(|| {
        value.as_object().and_then(|object| {
            ["name", "value", "level", "type"]
                .iter()
                .find_map(|key| object.get(*key).and_then(value_string))
        })
    })
}

fn value_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
}

fn normalize_model_key(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn api_model_keys(api_model: &LmStudioApiModel) -> Vec<String> {
    [
        api_model.id.clone(),
        api_model.display_name.clone(),
        api_model.path.clone(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|value| clean_string(Some(value)))
    .collect()
}

fn capability_names(value: &Value) -> Vec<String> {
    if let Some(capabilities) = value.as_array() {
        return capabilities
            .iter()
            .filter_map(Value::as_str)
            .map(|capability| capability.trim().to_ascii_lowercase())
            .filter(|capability| !capability.is_empty())
            .collect();
    }

    if let Some(capabilities) = value.as_object() {
        return capabilities
            .iter()
            .filter_map(|(key, value)| match value {
                Value::Bool(true) => Some(key.trim().to_ascii_lowercase()),
                Value::String(value) if value.eq_ignore_ascii_case("true") => {
                    Some(key.trim().to_ascii_lowercase())
                }
                _ => None,
            })
            .filter(|capability| !capability.is_empty())
            .collect();
    }

    Vec::new()
}

#[derive(Debug, Deserialize)]
struct LmStudioApiModelsResponse {
    #[serde(default)]
    data: Vec<LmStudioApiModel>,
    #[serde(default)]
    models: Vec<LmStudioApiModel>,
}

impl LmStudioApiModelsResponse {
    fn into_models(self) -> Vec<LmStudioApiModel> {
        if self.data.is_empty() {
            self.models
        } else {
            self.data
        }
    }
}

#[derive(Debug, Deserialize)]
struct LmStudioApiModel {
    #[serde(default, alias = "key")]
    id: Option<String>,
    #[serde(default, alias = "display_name", alias = "name")]
    display_name: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, alias = "owned_by")]
    publisher: Option<String>,
    #[serde(default, alias = "arch", alias = "model_architecture")]
    architecture: Option<String>,
    #[serde(default, alias = "compatibility_type")]
    format: Option<String>,
    #[serde(default)]
    quantization: Option<Value>,
    #[serde(default, alias = "parameter_size", alias = "params")]
    parameter_size: Option<Value>,
    #[serde(default, alias = "parameter_count")]
    parameter_count: Option<Value>,
    #[serde(default, alias = "max_context_length", alias = "context_length")]
    max_context_length: Option<Value>,
    #[serde(default, alias = "size_bytes", alias = "size")]
    size_bytes: Option<Value>,
    #[serde(default, alias = "type")]
    model_type: Option<String>,
    #[serde(default)]
    capabilities: Option<Value>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default, alias = "loaded_instances")]
    loaded_instances: Vec<Value>,
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
    fn enriches_scanned_model_from_lm_studio_api_metadata() {
        let model_dir = fixture_root()
            .join("lmstudio-community")
            .join("Qwen3-4B-GGUF");
        let mut model = scan_model_dir(&model_dir, "lmstudio-community", "Qwen3-4B-GGUF")
            .expect("model dir should scan")
            .model
            .expect("model should be present");
        let api_response = parse_api_models_response(
            r#"{
                "models": [
                    {
                        "key": "lmstudio-community/Qwen3-4B-GGUF",
                        "display_name": "Qwen3-4B-GGUF",
                        "publisher": "lmstudio-community",
                        "architecture": "qwen3",
                        "compatibility_type": "gguf",
                        "quantization": { "name": "Q4_K_M" },
                        "parameter_size": "4B",
                        "parameter_count": 4022000000,
                        "max_context_length": 40960,
                        "size": 123,
                        "type": "llm",
                        "capabilities": { "tools": true, "vision": true },
                        "loaded_instances": [{ "identifier": "loaded-1" }]
                    }
                ]
            }"#,
        )
        .expect("LM Studio API response should parse")
        .into_models();

        enrich_models_from_api(std::slice::from_mut(&mut model), &api_response);

        assert_eq!(model.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(model.parameter_size.as_deref(), Some("4B"));
        assert_eq!(model.technical.architecture.as_deref(), Some("qwen3"));
        assert_eq!(model.technical.parameter_count, Some(4_022_000_000));
        assert_eq!(model.technical.max_context_length, Some(40_960));
        assert_eq!(model.technical.context_length, Some(40_960));
        assert_eq!(model.capabilities.tool_use, Some(true));
        assert_eq!(model.capabilities.vision, Some(true));
        assert_eq!(model.runtime_status, Some(ModelRuntimeStatus::Loaded));
        assert_ne!(model.size_bytes, Some(123));
        assert!(model
            .metadata_sources
            .iter()
            .any(|source| source == "lmstudio_api"));
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
