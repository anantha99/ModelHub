use std::collections::HashMap;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{
    LocalModel, LocalModelCapabilities, LocalModelProvenance, LocalModelTechnical, ModelFormat,
    ModelRuntimeStatus, ModelSource,
};

use super::common::parse_quantization_from_name;

pub const OLLAMA_BASE_URL: &str = "http://localhost:11434";
const OLLAMA_TIMEOUT: Duration = Duration::from_millis(1_500);

#[derive(Debug)]
pub struct OllamaScanReport {
    pub models: Vec<LocalModel>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OllamaScanError {
    NotRunning,
    Request(String),
    ApiStatus(u16),
    Decode(String),
}

impl OllamaScanError {
    pub fn user_message(&self) -> String {
        match self {
            Self::NotRunning => "Ollama is not running on localhost:11434.".to_string(),
            Self::Request(message) => {
                format!("Could not check Ollama on localhost:11434: {message}")
            }
            Self::ApiStatus(status) => {
                format!("Ollama returned an unexpected HTTP status: {status}.")
            }
            Self::Decode(message) => {
                format!("Ollama returned model data ModelHub could not read: {message}")
            }
        }
    }
}

pub fn scan_models() -> Result<OllamaScanReport, OllamaScanError> {
    let client = Client::builder()
        .timeout(OLLAMA_TIMEOUT)
        .build()
        .map_err(|error| OllamaScanError::Request(error.to_string()))?;

    scan_models_with_client(&client, OLLAMA_BASE_URL)
}

fn scan_models_with_client(
    client: &Client,
    base_url: &str,
) -> Result<OllamaScanReport, OllamaScanError> {
    let response = client
        .get(format!("{base_url}/api/tags"))
        .send()
        .map_err(classify_request_error)?;

    if !response.status().is_success() {
        return Err(OllamaScanError::ApiStatus(response.status().as_u16()));
    }

    let body = response
        .text()
        .map_err(|error| OllamaScanError::Request(error.to_string()))?;

    let mut report = parse_tags_response(&body)?;
    enrich_models_with_show(client, base_url, &mut report.models);

    Ok(report)
}

fn classify_request_error(error: reqwest::Error) -> OllamaScanError {
    if error.is_connect() || error.is_timeout() {
        OllamaScanError::NotRunning
    } else {
        OllamaScanError::Request(error.to_string())
    }
}

fn parse_tags_response(body: &str) -> Result<OllamaScanReport, OllamaScanError> {
    let response = serde_json::from_str::<OllamaTagsResponse>(body)
        .map_err(|error| OllamaScanError::Decode(error.to_string()))?;
    let mut models = response
        .models
        .into_iter()
        .filter_map(local_model_from_ollama_model)
        .collect::<Vec<_>>();

    models.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(OllamaScanReport { models })
}

fn enrich_models_with_show(client: &Client, base_url: &str, models: &mut [LocalModel]) {
    for model in models {
        let Some(name) = model.repo_id.clone() else {
            continue;
        };

        let Ok(show_response) = fetch_show_response(client, base_url, &name) else {
            continue;
        };

        apply_show_response(model, show_response);
    }
}

fn fetch_show_response(
    client: &Client,
    base_url: &str,
    name: &str,
) -> Result<OllamaShowResponse, OllamaScanError> {
    let response = client
        .post(format!("{base_url}/api/show"))
        .json(&OllamaShowRequest { model: name })
        .send()
        .map_err(classify_request_error)?;

    if !response.status().is_success() {
        return Err(OllamaScanError::ApiStatus(response.status().as_u16()));
    }

    let body = response
        .text()
        .map_err(|error| OllamaScanError::Request(error.to_string()))?;

    parse_show_response(&body)
}

fn parse_show_response(body: &str) -> Result<OllamaShowResponse, OllamaScanError> {
    serde_json::from_str::<OllamaShowResponse>(body)
        .map_err(|error| OllamaScanError::Decode(error.to_string()))
}

fn apply_show_response(model: &mut LocalModel, response: OllamaShowResponse) {
    let mut changed = false;

    if let Some(details) = response.details {
        changed |= merge_details(model, details);
    }

    if let Some(model_info) = response.model_info {
        changed |= merge_model_info(model, &model_info);
    }

    if let Some(capabilities) = response.capabilities {
        changed |= merge_capabilities(model, capabilities);
    }

    if changed {
        push_unique_string(&mut model.metadata_sources, "ollama_show".to_string());
    }
}

fn merge_details(model: &mut LocalModel, details: OllamaModelDetails) -> bool {
    let mut changed = false;
    let format = model_format_from_ollama(details.format.as_deref());

    if format != ModelFormat::Unknown
        && (model.format.is_none() || model.format == Some(ModelFormat::Unknown))
    {
        model.format = Some(format);
        changed = true;
    }

    changed |= set_option_if_missing(
        &mut model.quantization,
        details
            .quantization_level
            .and_then(blank_to_none)
            .map(|quantization| quantization.to_ascii_uppercase()),
    );

    let parameter_size = details.parameter_size.and_then(blank_to_none);
    changed |= set_option_if_missing(&mut model.parameter_size, parameter_size.clone());
    changed |= set_option_if_missing(&mut model.technical.parameter_size, parameter_size);

    let family = details.family.and_then(blank_to_none);
    changed |= set_option_if_missing(&mut model.technical.family, family.clone());
    changed |= set_option_if_missing(&mut model.technical.architecture, family.clone());

    if let Some(family) = family {
        changed |= push_unique_string(&mut model.technical.families, family);
    }

    for family in details
        .families
        .unwrap_or_default()
        .into_iter()
        .filter_map(blank_to_none)
    {
        changed |= push_unique_string(&mut model.technical.families, family);
    }

    changed
}

fn merge_model_info(model: &mut LocalModel, model_info: &HashMap<String, Value>) -> bool {
    let mut changed = false;
    let architecture = json_string(model_info, "general.architecture");

    changed |= set_option_if_missing(&mut model.technical.architecture, architecture.clone());
    changed |= set_option_if_missing(&mut model.technical.family, architecture.clone());

    if let Some(architecture) = architecture {
        changed |= push_unique_string(&mut model.technical.families, architecture);
    }

    let architecture = model.technical.architecture.as_deref();

    changed |= set_option_if_missing(
        &mut model.technical.parameter_count,
        json_u64(model_info, "general.parameter_count"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.context_length,
        architecture_json_u64(model_info, architecture, "context_length"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.max_context_length,
        model.technical.context_length,
    );
    changed |= set_option_if_missing(
        &mut model.technical.embedding_length,
        architecture_json_u64(model_info, architecture, "embedding_length"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.block_count,
        architecture_json_u64(model_info, architecture, "block_count"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.attention_heads,
        architecture_json_u64(model_info, architecture, "attention.head_count"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.kv_heads,
        architecture_json_u64(model_info, architecture, "attention.head_count_kv"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.vocab_size,
        architecture_json_u64(model_info, architecture, "vocab_size"),
    );
    changed |= set_option_if_missing(
        &mut model.technical.tokenizer,
        json_string(model_info, "tokenizer.ggml.model"),
    );

    changed
}

fn merge_capabilities(model: &mut LocalModel, capabilities: Vec<String>) -> bool {
    let mut changed = false;
    let normalized = capabilities
        .iter()
        .map(|capability| capability.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();

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

fn local_model_from_ollama_model(model: OllamaModel) -> Option<LocalModel> {
    let name = blank_to_none(model.name)?;
    let details = model.details.unwrap_or_default();
    let quantization = details
        .quantization_level
        .clone()
        .and_then(blank_to_none)
        .map(|quantization| quantization.to_ascii_uppercase())
        .or_else(|| parse_quantization_from_name(&name));
    let parameter_size = details.parameter_size.clone().and_then(blank_to_none);
    let family = details.family.clone().and_then(blank_to_none);
    let families = details
        .families
        .unwrap_or_default()
        .into_iter()
        .filter_map(blank_to_none)
        .collect::<Vec<_>>();
    let format = model_format_from_ollama(details.format.as_deref());
    let mut metadata_sources = vec!["ollama_tags".to_string()];

    if model
        .digest
        .as_ref()
        .and_then(|digest| blank_to_none(digest.clone()))
        .is_some()
    {
        metadata_sources.push("ollama_digest".to_string());
    }

    Some(LocalModel {
        id: format!("ollama:{name}"),
        display_name: name.clone(),
        provider: provider_from_model_name(&name),
        repo_id: model.model.and_then(blank_to_none).or(Some(name)),
        source: ModelSource::Ollama,
        path: None,
        size_bytes: model.size.filter(|size| *size > 0),
        format: Some(format),
        quantization,
        parameter_size: parameter_size.clone(),
        last_modified: model.modified_at.and_then(blank_to_none),
        files: Vec::new(),
        runtime_status: Some(ModelRuntimeStatus::Available),
        technical: LocalModelTechnical {
            architecture: family.clone(),
            family,
            families,
            parameter_size,
            ..Default::default()
        },
        capabilities: LocalModelCapabilities::default(),
        provenance: LocalModelProvenance {
            digest: model.digest.and_then(blank_to_none),
            ..Default::default()
        },
        metadata_sources,
    })
}

fn provider_from_model_name(name: &str) -> Option<String> {
    name.split_once('/')
        .map(|(provider, _model)| provider)
        .filter(|provider| !provider.is_empty())
        .map(ToString::to_string)
}

fn model_format_from_ollama(format: Option<&str>) -> ModelFormat {
    match format.map(|format| format.to_ascii_lowercase()).as_deref() {
        Some("gguf") => ModelFormat::Gguf,
        Some("safetensors") => ModelFormat::Safetensors,
        Some("onnx") => ModelFormat::Onnx,
        Some("mlx") => ModelFormat::Mlx,
        _ => ModelFormat::Unknown,
    }
}

fn blank_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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

fn json_string(values: &HashMap<String, Value>, key: &str) -> Option<String> {
    values
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn json_u64(values: &HashMap<String, Value>, key: &str) -> Option<u64> {
    values.get(key).and_then(value_u64)
}

fn architecture_json_u64(
    values: &HashMap<String, Value>,
    architecture: Option<&str>,
    suffix: &str,
) -> Option<u64> {
    architecture.and_then(|architecture| json_u64(values, &format!("{architecture}.{suffix}")))
}

fn value_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
}

#[derive(Debug, Serialize)]
struct OllamaShowRequest<'a> {
    model: &'a str,
}

#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    details: Option<OllamaModelDetails>,
    model_info: Option<HashMap<String, Value>>,
    capabilities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    model: Option<String>,
    modified_at: Option<String>,
    size: Option<u64>,
    digest: Option<String>,
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    format: Option<String>,
    family: Option<String>,
    families: Option<Vec<String>>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installed_models_from_tags_response() {
        let report = parse_tags_response(
            r#"{
                "models": [
                    {
                        "name": "llama3.2:latest",
                        "modified_at": "2024-09-25T19:22:00Z",
                        "size": 2019393189,
                        "details": {
                            "format": "gguf",
                            "parameter_size": "3.2B",
                            "quantization_level": "Q4_K_M"
                        }
                    }
                ]
            }"#,
        )
        .expect("tags response should parse");

        assert_eq!(report.models.len(), 1);
        let model = &report.models[0];

        assert_eq!(model.id, "ollama:llama3.2:latest");
        assert_eq!(model.display_name, "llama3.2:latest");
        assert_eq!(model.source, ModelSource::Ollama);
        assert_eq!(model.repo_id.as_deref(), Some("llama3.2:latest"));
        assert_eq!(model.provider, None);
        assert_eq!(model.path, None);
        assert_eq!(model.size_bytes, Some(2_019_393_189));
        assert_eq!(model.format, Some(ModelFormat::Gguf));
        assert_eq!(model.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(model.parameter_size.as_deref(), Some("3.2B"));
        assert_eq!(model.last_modified.as_deref(), Some("2024-09-25T19:22:00Z"));
        assert!(model.files.is_empty());
        assert_eq!(model.runtime_status, Some(ModelRuntimeStatus::Available));
    }

    #[test]
    fn applies_show_metadata_without_replacing_known_tag_fields() {
        let mut model = parse_tags_response(
            r#"{
                "models": [
                    {
                        "name": "qwen3:latest",
                        "size": 5220000000,
                        "details": {
                            "format": "gguf",
                            "family": "qwen3",
                            "parameter_size": "4B",
                            "quantization_level": "Q4_K_M"
                        }
                    }
                ]
            }"#,
        )
        .expect("tags response should parse")
        .models
        .remove(0);

        let show_response = parse_show_response(
            r#"{
                "details": {
                    "format": "gguf",
                    "family": "qwen3",
                    "families": ["qwen3", "qwen"],
                    "parameter_size": "4B",
                    "quantization_level": "Q8_0"
                },
                "capabilities": ["completion", "tools"],
                "model_info": {
                    "general.architecture": "qwen3",
                    "general.parameter_count": 4022000000,
                    "qwen3.context_length": 40960,
                    "qwen3.embedding_length": 2560,
                    "qwen3.block_count": 36,
                    "qwen3.attention.head_count": 32,
                    "qwen3.attention.head_count_kv": 8,
                    "qwen3.vocab_size": 151936,
                    "tokenizer.ggml.model": "gpt2"
                }
            }"#,
        )
        .expect("show response should parse");

        apply_show_response(&mut model, show_response);

        assert_eq!(model.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(model.parameter_size.as_deref(), Some("4B"));
        assert_eq!(model.technical.architecture.as_deref(), Some("qwen3"));
        assert_eq!(model.technical.parameter_count, Some(4_022_000_000));
        assert_eq!(model.technical.context_length, Some(40_960));
        assert_eq!(model.technical.max_context_length, Some(40_960));
        assert_eq!(model.technical.embedding_length, Some(2_560));
        assert_eq!(model.technical.block_count, Some(36));
        assert_eq!(model.technical.attention_heads, Some(32));
        assert_eq!(model.technical.kv_heads, Some(8));
        assert_eq!(model.technical.vocab_size, Some(151_936));
        assert_eq!(model.technical.tokenizer.as_deref(), Some("gpt2"));
        assert!(model
            .technical
            .families
            .iter()
            .any(|family| family == "qwen"));
        assert_eq!(model.capabilities.tool_use, Some(true));
        assert!(model
            .metadata_sources
            .iter()
            .any(|source| source == "ollama_show"));
    }

    #[test]
    fn show_request_uses_ollama_model_field() {
        let value = serde_json::to_value(OllamaShowRequest {
            model: "qwen3:latest",
        })
        .expect("show request should serialize");

        assert_eq!(value, serde_json::json!({ "model": "qwen3:latest" }));
    }

    #[test]
    fn empty_models_response_is_ok() {
        let report = parse_tags_response(r#"{"models": []}"#).expect("empty response should parse");

        assert!(report.models.is_empty());
    }

    #[test]
    fn skips_blank_model_names_and_sorts_models() {
        let report = parse_tags_response(
            r#"{
                "models": [
                    { "name": "zeta:latest", "size": 10, "details": { "format": "gguf" } },
                    { "name": " ", "size": 10, "details": { "format": "gguf" } },
                    { "name": "library/alpha:q8_0", "size": 20, "details": { "format": "GGUF" } }
                ]
            }"#,
        )
        .expect("tags response should parse");

        assert_eq!(report.models.len(), 2);
        assert_eq!(report.models[0].id, "ollama:library/alpha:q8_0");
        assert_eq!(report.models[0].provider.as_deref(), Some("library"));
        assert_eq!(report.models[0].quantization.as_deref(), Some("Q8_0"));
        assert_eq!(report.models[1].id, "ollama:zeta:latest");
    }

    #[test]
    fn invalid_json_returns_decode_error() {
        let error = parse_tags_response("not json").expect_err("invalid json should fail");

        assert!(matches!(error, OllamaScanError::Decode(_)));
    }
}
