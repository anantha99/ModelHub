use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub hf_cache_path: Option<String>,
    pub lm_studio_models_path: Option<String>,
    pub custom_model_folders: Vec<String>,
    pub hf_token_stored: bool,
    pub minimize_to_tray: bool,
    pub start_on_login: bool,
    pub enable_symlink_attempt: bool,
    pub scan_on_startup: bool,
    pub delete_uses_recycle_bin: bool,
    pub telemetry_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hf_cache_path: None,
            lm_studio_models_path: None,
            custom_model_folders: Vec::new(),
            hf_token_stored: false,
            minimize_to_tray: true,
            start_on_login: false,
            enable_symlink_attempt: true,
            scan_on_startup: true,
            delete_uses_recycle_bin: true,
            telemetry_enabled: false,
        }
    }
}

impl AppSettings {
    pub fn sanitized(mut self) -> Self {
        self.hf_cache_path = sanitize_optional_path(self.hf_cache_path.as_deref());
        self.lm_studio_models_path = sanitize_optional_path(self.lm_studio_models_path.as_deref());
        self.custom_model_folders = sanitize_path_list(&self.custom_model_folders);
        self.hf_token_stored = false;
        self.delete_uses_recycle_bin = true;
        self.telemetry_enabled = false;
        self
    }

    pub fn apply_patch(&mut self, patch: AppSettingsPatch) {
        match patch.hf_cache_path {
            NullableStringPatch::Missing => {}
            NullableStringPatch::Clear => self.hf_cache_path = None,
            NullableStringPatch::Set(path) => {
                self.hf_cache_path = sanitize_optional_path(Some(&path))
            }
        }

        match patch.lm_studio_models_path {
            NullableStringPatch::Missing => {}
            NullableStringPatch::Clear => self.lm_studio_models_path = None,
            NullableStringPatch::Set(path) => {
                self.lm_studio_models_path = sanitize_optional_path(Some(&path));
            }
        }

        if let Some(folders) = patch.custom_model_folders {
            self.custom_model_folders = sanitize_path_list(&folders);
        }

        if let Some(value) = patch.minimize_to_tray {
            self.minimize_to_tray = value;
        }

        if let Some(value) = patch.start_on_login {
            self.start_on_login = value;
        }

        if let Some(value) = patch.enable_symlink_attempt {
            self.enable_symlink_attempt = value;
        }

        if let Some(value) = patch.scan_on_startup {
            self.scan_on_startup = value;
        }

        if let Some(value) = patch.delete_uses_recycle_bin {
            self.delete_uses_recycle_bin = value;
        }

        if patch.hf_token_stored.is_some() {
            self.hf_token_stored = false;
        }

        if patch.telemetry_enabled.is_some() {
            self.telemetry_enabled = false;
        }

        self.hf_token_stored = false;
        self.delete_uses_recycle_bin = true;
        self.telemetry_enabled = false;
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    #[serde(default)]
    pub hf_cache_path: NullableStringPatch,
    #[serde(default)]
    pub lm_studio_models_path: NullableStringPatch,
    #[serde(default)]
    pub custom_model_folders: Option<Vec<String>>,
    #[serde(default)]
    pub hf_token_stored: Option<bool>,
    #[serde(default)]
    pub minimize_to_tray: Option<bool>,
    #[serde(default)]
    pub start_on_login: Option<bool>,
    #[serde(default)]
    pub enable_symlink_attempt: Option<bool>,
    #[serde(default)]
    pub scan_on_startup: Option<bool>,
    #[serde(default)]
    pub delete_uses_recycle_bin: Option<bool>,
    #[serde(default)]
    pub telemetry_enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum NullableStringPatch {
    #[default]
    Missing,
    Clear,
    Set(String),
}

impl<'de> Deserialize<'de> for NullableStringPatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_option(NullableStringPatchVisitor)
    }
}

struct NullableStringPatchVisitor;

impl<'de> Visitor<'de> for NullableStringPatchVisitor {
    type Value = NullableStringPatch;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string path, null, or an omitted field")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(NullableStringPatch::Clear)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(NullableStringPatch::Set)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPaths {
    pub hf_cache: ResolvedPath,
    pub lm_studio_models: ResolvedPath,
    pub custom_model_folders: Vec<ResolvedPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPath {
    pub label: String,
    pub path: Option<String>,
    pub source: PathResolutionSource,
    pub source_label: String,
    pub exists: bool,
    pub is_directory: bool,
    pub issues: Vec<PathIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathResolutionSource {
    UserSetting,
    Environment,
    Default,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathIssue {
    pub severity: PathIssueSeverity,
    pub code: String,
    pub message: String,
}

impl PathIssue {
    pub fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: PathIssueSeverity::Warning,
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: PathIssueSeverity::Error,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub models: Vec<LocalModel>,
    pub source_statuses: Vec<SourceStatus>,
    pub total_size_bytes: Option<u64>,
    pub scanned_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: String,
    pub display_name: String,
    pub provider: Option<String>,
    pub repo_id: Option<String>,
    pub source: ModelSource,
    pub path: Option<String>,
    pub size_bytes: Option<u64>,
    pub format: Option<ModelFormat>,
    pub quantization: Option<String>,
    pub parameter_size: Option<String>,
    pub last_modified: Option<String>,
    pub files: Vec<LocalModelFile>,
    pub runtime_status: Option<ModelRuntimeStatus>,
    pub technical: LocalModelTechnical,
    pub capabilities: LocalModelCapabilities,
    pub provenance: LocalModelProvenance,
    pub metadata_sources: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelTechnical {
    pub architecture: Option<String>,
    pub family: Option<String>,
    pub families: Vec<String>,
    pub parameter_count: Option<u64>,
    pub parameter_size: Option<String>,
    pub context_length: Option<u64>,
    pub max_context_length: Option<u64>,
    pub embedding_length: Option<u64>,
    pub block_count: Option<u64>,
    pub attention_heads: Option<u64>,
    pub kv_heads: Option<u64>,
    pub vocab_size: Option<u64>,
    pub tokenizer: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelCapabilities {
    pub vision: Option<bool>,
    pub embedding: Option<bool>,
    pub tool_use: Option<bool>,
    pub reasoning: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelProvenance {
    pub digest: Option<String>,
    pub snapshot_sha: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    pub languages: Vec<String>,
    pub datasets: Vec<String>,
    pub base_models: Vec<String>,
    pub repo_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelFile {
    pub name: String,
    pub path: String,
    pub size_bytes: Option<u64>,
    pub format: ModelFormat,
    pub quantization: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum ModelSource {
    #[serde(rename = "huggingface")]
    HuggingFace,
    #[serde(rename = "lmstudio")]
    LmStudio,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "custom")]
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Gguf,
    Safetensors,
    Onnx,
    Mlx,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ModelRuntimeStatus {
    Available,
    Loaded,
    Running,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub source: ModelSource,
    pub status: SourceScanStatus,
    pub path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum SourceScanStatus {
    Ok,
    Missing,
    Error,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaRuntimeStatus {
    pub running: bool,
    pub base_url: String,
    pub models: Vec<LocalModel>,
    pub error: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu: Option<CpuInfo>,
    pub memory: MemoryInfo,
    pub gpus: Vec<GpuInfo>,
    pub hf_cache_disk: Option<DiskInfo>,
    pub collected_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: Option<String>,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HfSearchInput {
    pub query: String,
    #[serde(default)]
    pub filters: HfSearchFilters,
    pub sort: HfSearchSort,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HfSearchFilters {
    pub text_generation: bool,
    pub gguf: bool,
    pub safetensors: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HfSearchSort {
    Downloads,
    Likes,
    LastModified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfSearchResult {
    pub query: String,
    pub models: Vec<HfModelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfModelSummary {
    pub repo_id: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub downloads: Option<u64>,
    pub likes: Option<u64>,
    pub last_modified: Option<String>,
    pub gated: bool,
    pub private: bool,
    pub pipeline_tag: Option<String>,
    pub file_summary: HfFileSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfFileSummary {
    pub total_files: usize,
    pub gguf_files: usize,
    pub safetensors_files: usize,
    pub config_files: usize,
    pub tokenizer_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfModelDetails {
    pub repo_id: String,
    pub revision: String,
    pub commit_sha: Option<String>,
    pub gated: bool,
    pub private: bool,
    pub files: Vec<HfModelFile>,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HfModelFile {
    pub path: String,
    pub size_bytes: Option<u64>,
    pub format: ModelFormat,
    pub extension: Option<String>,
    pub lfs: bool,
    pub oid: Option<String>,
    pub blob_id: Option<String>,
    pub likely_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadInput {
    pub repo_id: String,
    pub revision: Option<String>,
    pub commit_sha: Option<String>,
    pub files: Vec<HfModelFile>,
    pub destination: DownloadDestination,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadDestination {
    Staging,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub id: String,
    pub repo_id: String,
    pub revision: String,
    pub commit_sha: Option<String>,
    pub destination: DownloadDestination,
    pub status: DownloadStatus,
    pub files: Vec<DownloadFileProgress>,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub error: Option<String>,
    pub installed_at: Option<String>,
    pub cache_path: Option<String>,
    pub snapshot_path: Option<String>,
    pub install_error: Option<String>,
    #[serde(default)]
    pub install_warnings: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFileProgress {
    pub path: String,
    pub size_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub staged_path: Option<String>,
    pub blob_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallDownloadResult {
    pub job_id: String,
    pub repo_id: String,
    pub cache_path: String,
    pub snapshot_path: String,
    pub installed_files: Vec<InstalledDownloadFile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledDownloadFile {
    pub path: String,
    pub blob_path: String,
    pub snapshot_path: String,
    pub linked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteModelInput {
    pub id: String,
    pub source: ModelSource,
    pub path: Option<String>,
    pub repo_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub deleted_path: String,
    pub used_recycle_bin: bool,
    pub message: String,
}

pub fn sanitize_optional_path(value: Option<&str>) -> Option<String> {
    value.and_then(|path| {
        let trimmed = trim_path_input(path);

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub fn sanitize_path_list(paths: &[String]) -> Vec<String> {
    let mut sanitized = Vec::new();
    let mut seen = Vec::new();

    for path in paths {
        let Some(path) = sanitize_optional_path(Some(path)) else {
            continue;
        };
        let key = normalize_path_key(&path);

        if seen.iter().any(|seen_key| seen_key == &key) {
            continue;
        }

        seen.push(key);
        sanitized.push(path);
    }

    sanitized
}

fn trim_path_input(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| character == '"' || character == '\'')
        .trim()
        .to_string()
}

fn normalize_path_key(path: &str) -> String {
    let normalized = path.replace('/', "\\").to_lowercase();
    let trimmed = normalized.trim_end_matches('\\');

    if trimmed.ends_with(':') {
        format!("{trimmed}\\")
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_keep_privacy_and_safety_defaults() {
        let settings = AppSettings::default();

        assert_eq!(settings.hf_cache_path, None);
        assert!(settings.minimize_to_tray);
        assert!(settings.enable_symlink_attempt);
        assert!(settings.scan_on_startup);
        assert!(settings.delete_uses_recycle_bin);
        assert!(!settings.hf_token_stored);
        assert!(!settings.telemetry_enabled);
    }

    #[test]
    fn patch_trims_and_clears_path_fields() {
        let mut settings = AppSettings::default();

        settings.apply_patch(AppSettingsPatch {
            hf_cache_path: NullableStringPatch::Set("  \"C:\\Models\\hf\"  ".to_string()),
            lm_studio_models_path: NullableStringPatch::Set("   ".to_string()),
            ..AppSettingsPatch::default()
        });

        assert_eq!(settings.hf_cache_path.as_deref(), Some("C:\\Models\\hf"));
        assert_eq!(settings.lm_studio_models_path, None);
    }

    #[test]
    fn patch_never_enables_telemetry_or_token_state() {
        let mut settings = AppSettings::default();

        settings.apply_patch(AppSettingsPatch {
            hf_token_stored: Some(true),
            delete_uses_recycle_bin: Some(false),
            telemetry_enabled: Some(true),
            ..AppSettingsPatch::default()
        });

        assert!(!settings.hf_token_stored);
        assert!(settings.delete_uses_recycle_bin);
        assert!(!settings.telemetry_enabled);
    }

    #[test]
    fn null_path_patch_clears_saved_value() {
        let patch = serde_json::from_value::<AppSettingsPatch>(serde_json::json!({
            "hfCachePath": null
        }))
        .expect("patch should deserialize");
        let mut settings = AppSettings {
            hf_cache_path: Some("C:\\Models\\hf".to_string()),
            ..AppSettings::default()
        };

        settings.apply_patch(patch);

        assert_eq!(settings.hf_cache_path, None);
    }

    #[test]
    fn missing_path_patch_preserves_saved_value() {
        let patch = serde_json::from_value::<AppSettingsPatch>(serde_json::json!({}))
            .expect("patch should deserialize");
        let mut settings = AppSettings {
            hf_cache_path: Some("C:\\Models\\hf".to_string()),
            ..AppSettings::default()
        };

        settings.apply_patch(patch);

        assert_eq!(settings.hf_cache_path.as_deref(), Some("C:\\Models\\hf"));
    }

    #[test]
    fn path_lists_drop_empty_and_duplicate_entries_case_insensitively() {
        let folders = sanitize_path_list(&[
            "C:\\Models".to_string(),
            " ".to_string(),
            "c:/models".to_string(),
            "C:\\Models\\".to_string(),
            "D:\\Models".to_string(),
        ]);

        assert_eq!(folders, vec!["C:\\Models", "D:\\Models"]);
    }
}
