use crate::models::OllamaRuntimeStatus;
use crate::scanner::ollama::{self, OllamaScanError, OllamaScanReport};

pub fn get_status() -> OllamaRuntimeStatus {
    status_from_scan_result(ollama::scan_models(), super::timestamp_now())
}

fn status_from_scan_result(
    result: Result<OllamaScanReport, OllamaScanError>,
    checked_at: String,
) -> OllamaRuntimeStatus {
    match result {
        Ok(report) => OllamaRuntimeStatus {
            running: true,
            base_url: ollama::OLLAMA_BASE_URL.to_string(),
            models: report.models,
            error: None,
            checked_at,
        },
        Err(error) => OllamaRuntimeStatus {
            running: false,
            base_url: ollama::OLLAMA_BASE_URL.to_string(),
            models: Vec::new(),
            error: Some(error.user_message()),
            checked_at,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LocalModel, ModelFormat, ModelRuntimeStatus, ModelSource};

    #[test]
    fn running_status_includes_models() {
        let status = status_from_scan_result(
            Ok(OllamaScanReport {
                models: vec![ollama_model("llama3.2:latest")],
            }),
            "123".to_string(),
        );

        assert!(status.running);
        assert_eq!(status.base_url, ollama::OLLAMA_BASE_URL);
        assert_eq!(status.models.len(), 1);
        assert_eq!(status.models[0].repo_id.as_deref(), Some("llama3.2:latest"));
        assert_eq!(status.error, None);
        assert_eq!(status.checked_at, "123");
    }

    #[test]
    fn not_running_status_is_not_an_error_stack() {
        let status = status_from_scan_result(Err(OllamaScanError::NotRunning), "123".to_string());

        assert!(!status.running);
        assert_eq!(status.base_url, ollama::OLLAMA_BASE_URL);
        assert!(status.models.is_empty());
        assert_eq!(
            status.error.as_deref(),
            Some("Ollama is not running on localhost:11434.")
        );
    }

    #[test]
    fn decode_errors_are_friendly_status_messages() {
        let status = status_from_scan_result(
            Err(OllamaScanError::Decode("expected value".to_string())),
            "123".to_string(),
        );

        assert!(!status.running);
        assert!(status.models.is_empty());
        assert!(status
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Ollama returned model data ModelHub could not read"));
    }

    #[test]
    fn empty_model_list_still_means_ollama_is_running() {
        let status = status_from_scan_result(
            Ok(OllamaScanReport { models: Vec::new() }),
            "123".to_string(),
        );

        assert!(status.running);
        assert!(status.models.is_empty());
        assert_eq!(status.error, None);
    }

    fn ollama_model(name: &str) -> LocalModel {
        LocalModel {
            id: format!("ollama:{name}"),
            display_name: name.to_string(),
            provider: None,
            repo_id: Some(name.to_string()),
            source: ModelSource::Ollama,
            path: None,
            size_bytes: Some(42),
            format: Some(ModelFormat::Gguf),
            quantization: Some("Q4_K_M".to_string()),
            parameter_size: Some("3.2B".to_string()),
            last_modified: Some("2024-09-25T19:22:00Z".to_string()),
            files: Vec::new(),
            runtime_status: Some(ModelRuntimeStatus::Available),
        }
    }
}
