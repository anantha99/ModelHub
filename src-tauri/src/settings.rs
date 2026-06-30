use std::fs;
use std::path::PathBuf;

use tauri::{Manager, Runtime};

use crate::errors::AppError;
use crate::models::{
    AppSettings, AppSettingsPatch, PathIssueSeverity, ResolvedPath, ResolvedPaths,
};
use crate::paths;

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn for_manager<R: Runtime, M: Manager<R>>(manager: &M) -> Result<Self, AppError> {
        let config_dir = manager
            .path()
            .app_config_dir()
            .map_err(AppError::SettingsDirectory)?;

        Ok(Self::new(config_dir.join(SETTINGS_FILE_NAME)))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<AppSettings, AppError> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }

        let contents = fs::read_to_string(&self.path).map_err(|source| AppError::ReadSettings {
            path: self.path.clone(),
            source,
        })?;

        let settings = serde_json::from_str::<AppSettings>(&contents).map_err(|source| {
            AppError::ParseSettings {
                path: self.path.clone(),
                source,
            }
        })?;

        Ok(settings.sanitized())
    }

    pub fn update(&self, patch: AppSettingsPatch) -> Result<AppSettings, AppError> {
        let mut settings = self.load()?;
        settings.apply_patch(patch);
        validate_settings_for_save(&settings)?;
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::CreateSettingsDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let settings = settings.clone().sanitized();
        let contents =
            serde_json::to_string_pretty(&settings).map_err(AppError::SerializeSettings)?;
        let temporary_path = self.path.with_extension("json.tmp");

        fs::write(&temporary_path, contents).map_err(|source| AppError::WriteSettings {
            path: temporary_path.clone(),
            source,
        })?;

        fs::rename(&temporary_path, &self.path).or_else(|rename_error| {
            if self.path.exists() {
                fs::remove_file(&self.path).map_err(|source| AppError::WriteSettings {
                    path: self.path.clone(),
                    source,
                })?;
                fs::rename(&temporary_path, &self.path).map_err(|source| AppError::WriteSettings {
                    path: self.path.clone(),
                    source,
                })
            } else {
                Err(AppError::WriteSettings {
                    path: self.path.clone(),
                    source: rename_error,
                })
            }
        })
    }

    pub fn minimize_to_tray(&self) -> bool {
        self.load()
            .map(|settings| settings.minimize_to_tray)
            .unwrap_or(true)
    }
}

fn validate_settings_for_save(settings: &AppSettings) -> Result<(), AppError> {
    let resolved_paths = paths::resolve_paths(settings);
    let errors = blocking_path_errors(&resolved_paths);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::InvalidSettings(errors))
    }
}

fn blocking_path_errors(paths: &ResolvedPaths) -> Vec<String> {
    let mut errors = Vec::new();

    collect_path_errors(&paths.hf_cache, &mut errors);
    collect_path_errors(&paths.lm_studio_models, &mut errors);

    for path in &paths.custom_model_folders {
        collect_path_errors(path, &mut errors);
    }

    errors
}

fn collect_path_errors(path: &ResolvedPath, errors: &mut Vec<String>) {
    for issue in &path.issues {
        if issue.severity == PathIssueSeverity::Error {
            errors.push(format!("{}: {}", path.label, issue.message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSettingsPatch, NullableStringPatch};

    fn settings_store() -> (tempfile::TempDir, SettingsStore) {
        let directory = tempfile::tempdir().expect("temp settings dir");
        let store = SettingsStore::new(directory.path().join("settings.json"));
        (directory, store)
    }

    #[test]
    fn missing_file_loads_defaults() {
        let (_directory, store) = settings_store();

        let settings = store.load().expect("settings should load");

        assert_eq!(settings, AppSettings::default());
    }

    #[test]
    fn settings_persist_and_reload() {
        let (_directory, store) = settings_store();
        let settings = AppSettings {
            hf_cache_path: Some("C:\\Models\\hf".to_string()),
            custom_model_folders: vec!["D:\\Models".to_string()],
            scan_on_startup: false,
            ..AppSettings::default()
        };

        store.save(&settings).expect("settings should save");
        let loaded = store.load().expect("settings should reload");

        assert_eq!(loaded.hf_cache_path.as_deref(), Some("C:\\Models\\hf"));
        assert_eq!(loaded.custom_model_folders, vec!["D:\\Models"]);
        assert!(!loaded.scan_on_startup);
    }

    #[test]
    fn partial_update_preserves_unspecified_values() {
        let (_directory, store) = settings_store();
        let original = AppSettings {
            hf_cache_path: Some("C:\\Models\\hf".to_string()),
            lm_studio_models_path: Some("C:\\Models\\lmstudio".to_string()),
            scan_on_startup: true,
            ..AppSettings::default()
        };

        store.save(&original).expect("settings should save");
        let updated = store
            .update(AppSettingsPatch {
                scan_on_startup: Some(false),
                ..AppSettingsPatch::default()
            })
            .expect("settings should update");

        assert_eq!(updated.hf_cache_path, original.hf_cache_path);
        assert_eq!(
            updated.lm_studio_models_path,
            original.lm_studio_models_path
        );
        assert!(!updated.scan_on_startup);
    }

    #[test]
    fn clearing_a_path_persists_as_unset() {
        let (_directory, store) = settings_store();

        store
            .update(AppSettingsPatch {
                hf_cache_path: NullableStringPatch::Set("C:\\Models\\hf".to_string()),
                ..AppSettingsPatch::default()
            })
            .expect("settings should update");
        let updated = store
            .update(AppSettingsPatch {
                hf_cache_path: NullableStringPatch::Clear,
                ..AppSettingsPatch::default()
            })
            .expect("settings should update");

        assert_eq!(updated.hf_cache_path, None);
    }

    #[test]
    fn update_rejects_relative_paths() {
        let (_directory, store) = settings_store();

        let result = store.update(AppSettingsPatch {
            hf_cache_path: NullableStringPatch::Set("models\\hf".to_string()),
            ..AppSettingsPatch::default()
        });

        assert!(matches!(result, Err(AppError::InvalidSettings(_))));
    }

    #[test]
    fn update_rejects_custom_drive_roots() {
        let (_directory, store) = settings_store();

        let result = store.update(AppSettingsPatch {
            custom_model_folders: Some(vec!["D:\\".to_string()]),
            ..AppSettingsPatch::default()
        });

        assert!(matches!(result, Err(AppError::InvalidSettings(_))));
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let directory = tempfile::tempdir().expect("temp settings dir");
        let store = SettingsStore::new(directory.path().join("nested").join("settings.json"));

        store
            .save(&AppSettings::default())
            .expect("settings should save to nested path");

        assert!(store.path.exists());
    }

    #[test]
    fn corrupt_json_returns_parse_error() {
        let (_directory, store) = settings_store();

        fs::write(&store.path, "not-json").expect("settings should write");

        assert!(matches!(store.load(), Err(AppError::ParseSettings { .. })));
    }

    #[test]
    fn saved_json_never_keeps_token_or_telemetry_enabled() {
        let (_directory, store) = settings_store();
        let settings = AppSettings {
            hf_token_stored: true,
            telemetry_enabled: true,
            ..AppSettings::default()
        };

        store.save(&settings).expect("settings should save");
        let contents = fs::read_to_string(&store.path).expect("settings file should exist");
        let saved = serde_json::from_str::<AppSettings>(&contents).expect("settings should parse");

        assert!(!saved.hf_token_stored);
        assert!(!saved.telemetry_enabled);
    }
}
