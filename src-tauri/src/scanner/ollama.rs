use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::models::{LocalModel, ModelFormat, ModelRuntimeStatus, ModelSource};

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

    parse_tags_response(&body)
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

fn local_model_from_ollama_model(model: OllamaModel) -> Option<LocalModel> {
    let name = blank_to_none(model.name)?;
    let details = model.details.unwrap_or_default();
    let quantization = details
        .quantization_level
        .and_then(blank_to_none)
        .map(|quantization| quantization.to_ascii_uppercase())
        .or_else(|| parse_quantization_from_name(&name));

    Some(LocalModel {
        id: format!("ollama:{name}"),
        display_name: name.clone(),
        provider: provider_from_model_name(&name),
        repo_id: Some(name),
        source: ModelSource::Ollama,
        path: None,
        size_bytes: model.size.filter(|size| *size > 0),
        format: Some(model_format_from_ollama(details.format.as_deref())),
        quantization,
        parameter_size: details.parameter_size.and_then(blank_to_none),
        last_modified: model.modified_at.and_then(blank_to_none),
        files: Vec::new(),
        runtime_status: Some(ModelRuntimeStatus::Available),
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

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    modified_at: Option<String>,
    size: Option<u64>,
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    format: Option<String>,
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
