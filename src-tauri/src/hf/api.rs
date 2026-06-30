use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::models::{
    HfFileSummary, HfModelDetails, HfModelFile, HfModelSummary, HfSearchInput, HfSearchResult,
    HfSearchSort, ModelFormat,
};

const HF_BASE_URL: &str = "https://huggingface.co";
const HF_SEARCH_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_SEARCH_LIMIT: u32 = 25;
const MAX_SEARCH_LIMIT: u32 = 50;

#[derive(Debug, PartialEq, Eq)]
pub enum HfApiError {
    InvalidInput(String),
    Request(String),
    ApiStatus(u16),
    Decode(String),
}

impl HfApiError {
    pub fn user_message(&self) -> String {
        match self {
            Self::InvalidInput(message) => message.clone(),
            Self::Request(message) => {
                format!("Could not search Hugging Face models: {message}")
            }
            Self::ApiStatus(status) => {
                format!("Hugging Face search returned an unexpected HTTP status: {status}.")
            }
            Self::Decode(message) => {
                format!("Hugging Face returned search data ModelHub could not read: {message}")
            }
        }
    }
}

pub fn get_model_details(
    repo_id: String,
    revision: Option<String>,
) -> Result<HfModelDetails, HfApiError> {
    let client = Client::builder()
        .timeout(HF_SEARCH_TIMEOUT)
        .user_agent("ModelHub-Windows/0.1")
        .build()
        .map_err(|error| HfApiError::Request(error.to_string()))?;

    get_model_details_with_client(&client, HF_BASE_URL, repo_id, revision)
}

fn get_model_details_with_client(
    client: &Client,
    base_url: &str,
    repo_id: String,
    revision: Option<String>,
) -> Result<HfModelDetails, HfApiError> {
    let repo_id = normalized_repo_id(&repo_id)?;
    let revision = revision
        .and_then(blank_to_none)
        .unwrap_or_else(|| "main".to_string());
    let url = build_details_url(base_url, &repo_id, &revision)?;
    let response = client.get(url).send().map_err(classify_request_error)?;

    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => HfApiError::Request(
                "This model is gated or private. Token support is not enabled for downloads yet."
                    .to_string(),
            ),
            404 => HfApiError::Request("Hugging Face could not find that model repo.".to_string()),
            status => HfApiError::ApiStatus(status),
        });
    }

    let body = response
        .text()
        .map_err(|error| HfApiError::Request(error.to_string()))?;

    parse_model_details_response(&repo_id, &revision, &body)
}

pub fn search_models(input: HfSearchInput) -> Result<HfSearchResult, HfApiError> {
    let client = Client::builder()
        .timeout(HF_SEARCH_TIMEOUT)
        .user_agent("ModelHub-Windows/0.1")
        .build()
        .map_err(|error| HfApiError::Request(error.to_string()))?;

    search_models_with_client(&client, HF_BASE_URL, input)
}

fn search_models_with_client(
    client: &Client,
    base_url: &str,
    input: HfSearchInput,
) -> Result<HfSearchResult, HfApiError> {
    let query = normalized_query(&input.query)?;
    let url = build_search_url(base_url, &input, &query)?;
    let response = client.get(url).send().map_err(classify_request_error)?;

    if !response.status().is_success() {
        return Err(HfApiError::ApiStatus(response.status().as_u16()));
    }

    let body = response
        .text()
        .map_err(|error| HfApiError::Request(error.to_string()))?;

    parse_search_response(&query, &body)
}

fn build_search_url(base_url: &str, input: &HfSearchInput, query: &str) -> Result<Url, HfApiError> {
    let mut url = Url::parse(base_url)
        .and_then(|base| base.join("/api/models"))
        .map_err(|error| HfApiError::Request(error.to_string()))?;
    let limit = input
        .limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);

    {
        let mut query_pairs = url.query_pairs_mut();

        query_pairs.append_pair("search", query);
        query_pairs.append_pair("limit", &limit.to_string());
        query_pairs.append_pair("full", "false");
        query_pairs.append_pair("sort", sort_param(&input.sort));
        query_pairs.append_pair("direction", "-1");

        if input.filters.text_generation {
            query_pairs.append_pair("pipeline_tag", "text-generation");
        }

        if input.filters.gguf {
            query_pairs.append_pair("filter", "gguf");
        }

        if input.filters.safetensors {
            query_pairs.append_pair("filter", "safetensors");
        }
    }

    Ok(url)
}

fn build_details_url(base_url: &str, repo_id: &str, revision: &str) -> Result<Url, HfApiError> {
    let mut url = Url::parse(base_url)
        .and_then(|base| base.join(&format!("/api/models/{repo_id}")))
        .map_err(|error| HfApiError::Request(error.to_string()))?;

    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs.append_pair("blobs", "true");
        query_pairs.append_pair("revision", revision);
    }

    Ok(url)
}

fn sort_param(sort: &HfSearchSort) -> &'static str {
    match sort {
        HfSearchSort::Downloads => "downloads",
        HfSearchSort::Likes => "likes",
        HfSearchSort::LastModified => "lastModified",
    }
}

fn normalized_query(query: &str) -> Result<String, HfApiError> {
    let query = query.trim();

    if query.is_empty() {
        Err(HfApiError::InvalidInput(
            "Enter a model name or keyword before searching Hugging Face.".to_string(),
        ))
    } else {
        Ok(query.to_string())
    }
}

fn normalized_repo_id(repo_id: &str) -> Result<String, HfApiError> {
    let repo_id = repo_id.trim();

    if repo_id.is_empty() || !repo_id.contains('/') || repo_id.contains("..") {
        Err(HfApiError::InvalidInput(
            "Choose a valid Hugging Face repo before loading model details.".to_string(),
        ))
    } else {
        Ok(repo_id.to_string())
    }
}

fn classify_request_error(error: reqwest::Error) -> HfApiError {
    if error.is_timeout() || error.is_connect() {
        HfApiError::Request("Hugging Face is not reachable right now.".to_string())
    } else {
        HfApiError::Request(error.to_string())
    }
}

fn parse_search_response(query: &str, body: &str) -> Result<HfSearchResult, HfApiError> {
    let models = serde_json::from_str::<Vec<HfApiModel>>(body)
        .map_err(|error| HfApiError::Decode(error.to_string()))?
        .into_iter()
        .filter_map(HfModelSummary::from_api_model)
        .collect::<Vec<_>>();

    Ok(HfSearchResult {
        query: query.to_string(),
        models,
    })
}

fn parse_model_details_response(
    fallback_repo_id: &str,
    revision: &str,
    body: &str,
) -> Result<HfModelDetails, HfApiError> {
    let model = serde_json::from_str::<HfApiModel>(body)
        .map_err(|error| HfApiError::Decode(error.to_string()))?;
    let repo_id = model
        .model_id
        .or(model.id)
        .and_then(blank_to_none)
        .unwrap_or_else(|| fallback_repo_id.to_string());
    let files = model
        .siblings
        .into_iter()
        .filter_map(HfModelFile::from_api_sibling)
        .collect::<Vec<_>>();
    let total_bytes = sum_known_sizes(&files);

    Ok(HfModelDetails {
        repo_id,
        revision: revision.to_string(),
        commit_sha: model.sha.and_then(blank_to_none),
        gated: value_is_gated(model.gated.as_ref()),
        private: model.private.unwrap_or(false),
        files,
        total_bytes,
    })
}

impl HfModelSummary {
    fn from_api_model(model: HfApiModel) -> Option<Self> {
        let repo_id = model.model_id.or(model.id).and_then(blank_to_none)?;
        let file_summary = summarize_files(&model.siblings);

        Some(Self {
            repo_id,
            author: model.author.and_then(blank_to_none),
            tags: model.tags,
            downloads: model.downloads,
            likes: model.likes,
            last_modified: model.last_modified.and_then(blank_to_none),
            gated: value_is_gated(model.gated.as_ref()),
            private: model.private.unwrap_or(false),
            pipeline_tag: model.pipeline_tag.and_then(blank_to_none),
            file_summary,
        })
    }
}

impl HfModelFile {
    fn from_api_sibling(sibling: HfApiSibling) -> Option<Self> {
        let path = sibling.rfilename.and_then(blank_to_none)?;
        let extension = file_extension(&path);
        let format = format_from_path(&path);
        let size_bytes = sibling
            .size
            .or_else(|| sibling.lfs.as_ref().and_then(|lfs| lfs.size));
        let oid = sibling.lfs.and_then(|lfs| lfs.oid.and_then(blank_to_none));
        let blob_id = oid
            .clone()
            .or_else(|| sibling.blob_id.and_then(blank_to_none));

        Some(Self {
            likely_default: is_likely_default_file(&path, &format),
            path,
            size_bytes,
            format,
            extension,
            lfs: oid.is_some(),
            oid,
            blob_id,
        })
    }
}

fn summarize_files(siblings: &[HfApiSibling]) -> HfFileSummary {
    let mut summary = HfFileSummary::default();

    for sibling in siblings {
        let Some(filename) = sibling.rfilename.as_deref() else {
            continue;
        };
        let lower_filename = filename.to_ascii_lowercase();

        summary.total_files += 1;

        if lower_filename.ends_with(".gguf") {
            summary.gguf_files += 1;
        }

        if lower_filename.ends_with(".safetensors") {
            summary.safetensors_files += 1;
        }

        if lower_filename.ends_with("config.json") {
            summary.config_files += 1;
        }

        if lower_filename.contains("tokenizer") {
            summary.tokenizer_files += 1;
        }
    }

    summary
}

fn value_is_gated(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(is_gated)) => *is_gated,
        Some(Value::String(value)) => !value.trim().is_empty() && value != "false",
        _ => false,
    }
}

fn sum_known_sizes(files: &[HfModelFile]) -> Option<u64> {
    let mut total = 0_u64;

    for file in files {
        total = total.checked_add(file.size_bytes?)?;
    }

    Some(total)
}

fn file_extension(path: &str) -> Option<String> {
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .and_then(blank_to_none)
}

fn format_from_path(path: &str) -> ModelFormat {
    let lower_path = path.to_ascii_lowercase();

    if lower_path.ends_with(".gguf") {
        ModelFormat::Gguf
    } else if lower_path.ends_with(".safetensors") {
        ModelFormat::Safetensors
    } else if lower_path.ends_with(".onnx") {
        ModelFormat::Onnx
    } else if lower_path.ends_with(".mlx") {
        ModelFormat::Mlx
    } else {
        ModelFormat::Unknown
    }
}

fn is_likely_default_file(path: &str, format: &ModelFormat) -> bool {
    let lower_path = path.to_ascii_lowercase();

    matches!(format, ModelFormat::Gguf)
        || lower_path.ends_with("config.json")
        || lower_path.contains("tokenizer")
        || lower_path.ends_with("tokenizer.model")
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
#[serde(rename_all = "camelCase")]
struct HfApiModel {
    id: Option<String>,
    model_id: Option<String>,
    author: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    downloads: Option<u64>,
    likes: Option<u64>,
    last_modified: Option<String>,
    gated: Option<Value>,
    private: Option<bool>,
    #[serde(alias = "pipeline_tag")]
    pipeline_tag: Option<String>,
    sha: Option<String>,
    #[serde(default)]
    siblings: Vec<HfApiSibling>,
}

#[derive(Debug, Deserialize)]
struct HfApiSibling {
    rfilename: Option<String>,
    size: Option<u64>,
    #[serde(alias = "blobId")]
    blob_id: Option<String>,
    lfs: Option<HfApiLfs>,
}

#[derive(Debug, Deserialize)]
struct HfApiLfs {
    oid: Option<String>,
    size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HfSearchFilters;

    #[test]
    fn rejects_blank_queries_before_network() {
        let error = normalized_query("  ").expect_err("blank query should fail");

        assert_eq!(
            error.user_message(),
            "Enter a model name or keyword before searching Hugging Face."
        );
    }

    #[test]
    fn builds_search_url_with_filters_and_sort() {
        let input = HfSearchInput {
            query: " qwen ".to_string(),
            filters: HfSearchFilters {
                text_generation: true,
                gguf: true,
                safetensors: true,
            },
            sort: HfSearchSort::LastModified,
            limit: Some(100),
        };
        let url = build_search_url(HF_BASE_URL, &input, "qwen").expect("url should build");
        let query = url.query().expect("url should have query params");

        assert_eq!(url.path(), "/api/models");
        assert!(query.contains("search=qwen"));
        assert!(query.contains("limit=50"));
        assert!(query.contains("sort=lastModified"));
        assert!(query.contains("direction=-1"));
        assert!(query.contains("pipeline_tag=text-generation"));
        assert!(query.contains("filter=gguf"));
        assert!(query.contains("filter=safetensors"));
    }

    #[test]
    fn parses_search_response_with_model_id_and_file_summary() {
        let result = parse_search_response(
            "qwen",
            r#"[
                {
                    "id": "unused-id",
                    "modelId": "Qwen/Qwen3-4B",
                    "author": "Qwen",
                    "gated": false,
                    "private": false,
                    "lastModified": "2025-07-26T03:46:39.000Z",
                    "likes": 641,
                    "downloads": 16369272,
                    "tags": ["safetensors", "text-generation"],
                    "pipeline_tag": "text-generation",
                    "siblings": [
                        { "rfilename": "config.json" },
                        { "rfilename": "model-00001-of-00003.safetensors" },
                        { "rfilename": "tokenizer.json" }
                    ]
                }
            ]"#,
        )
        .expect("search response should parse");

        assert_eq!(result.query, "qwen");
        assert_eq!(result.models.len(), 1);
        let model = &result.models[0];

        assert_eq!(model.repo_id, "Qwen/Qwen3-4B");
        assert_eq!(model.author.as_deref(), Some("Qwen"));
        assert!(!model.gated);
        assert!(!model.private);
        assert_eq!(model.downloads, Some(16_369_272));
        assert_eq!(model.likes, Some(641));
        assert_eq!(model.pipeline_tag.as_deref(), Some("text-generation"));
        assert_eq!(model.file_summary.total_files, 3);
        assert_eq!(model.file_summary.safetensors_files, 1);
        assert_eq!(model.file_summary.config_files, 1);
        assert_eq!(model.file_summary.tokenizer_files, 1);
    }

    #[test]
    fn parses_model_details_with_lfs_metadata_and_sizes() {
        let details = parse_model_details_response(
            "Qwen/Qwen3-4B-GGUF",
            "main",
            r#"{
                "modelId": "Qwen/Qwen3-4B-GGUF",
                "sha": "abc123",
                "gated": false,
                "private": false,
                "siblings": [
                    {
                        "rfilename": "Qwen3-4B-Q4_K_M.gguf",
                        "lfs": { "oid": "blob-a", "size": 4096 }
                    },
                    { "rfilename": "config.json", "size": 128 },
                    { "rfilename": "README.md" }
                ]
            }"#,
        )
        .expect("details response should parse");

        assert_eq!(details.repo_id, "Qwen/Qwen3-4B-GGUF");
        assert_eq!(details.commit_sha.as_deref(), Some("abc123"));
        assert_eq!(details.total_bytes, None);
        assert_eq!(details.files.len(), 3);

        let gguf = &details.files[0];
        assert_eq!(gguf.path, "Qwen3-4B-Q4_K_M.gguf");
        assert_eq!(gguf.size_bytes, Some(4096));
        assert_eq!(gguf.format, ModelFormat::Gguf);
        assert_eq!(gguf.extension.as_deref(), Some("gguf"));
        assert!(gguf.lfs);
        assert_eq!(gguf.oid.as_deref(), Some("blob-a"));
        assert_eq!(gguf.blob_id.as_deref(), Some("blob-a"));
        assert!(gguf.likely_default);

        let config = &details.files[1];
        assert_eq!(config.size_bytes, Some(128));
        assert_eq!(config.format, ModelFormat::Unknown);
        assert!(config.likely_default);
    }

    #[test]
    fn parses_non_lfs_blob_id_from_details() {
        let details = parse_model_details_response(
            "Qwen/Qwen3-4B",
            "main",
            r#"{
                "modelId": "Qwen/Qwen3-4B",
                "siblings": [
                    { "rfilename": "config.json", "size": 128, "blobId": "plain-blob" }
                ]
            }"#,
        )
        .expect("details response should parse");

        assert_eq!(details.files[0].blob_id.as_deref(), Some("plain-blob"));
        assert_eq!(details.files[0].oid, None);
        assert!(!details.files[0].lfs);
    }

    #[test]
    fn totals_known_sizes_only_when_every_file_has_size() {
        let details = parse_model_details_response(
            "Qwen/Qwen3-4B",
            "main",
            r#"{
                "modelId": "Qwen/Qwen3-4B",
                "siblings": [
                    { "rfilename": "config.json", "size": 100 },
                    { "rfilename": "tokenizer.json", "size": 250 }
                ]
            }"#,
        )
        .expect("details response should parse");

        assert_eq!(details.total_bytes, Some(350));
    }

    #[test]
    fn parses_gated_values_from_boolean_or_string() {
        assert!(!value_is_gated(Some(&serde_json::json!(false))));
        assert!(value_is_gated(Some(&serde_json::json!(true))));
        assert!(value_is_gated(Some(&serde_json::json!("auto"))));
        assert!(value_is_gated(Some(&serde_json::json!("manual"))));
        assert!(!value_is_gated(Some(&serde_json::json!("false"))));
        assert!(!value_is_gated(None));
    }

    #[test]
    fn invalid_json_returns_decode_error() {
        let error =
            parse_search_response("qwen", "not json").expect_err("invalid json should fail");

        assert!(matches!(error, HfApiError::Decode(_)));
    }
}
