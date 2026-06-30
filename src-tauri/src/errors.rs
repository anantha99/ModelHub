use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("could not resolve the app settings directory")]
    SettingsDirectory(#[source] tauri::Error),
    #[error("could not create the app settings directory at {path}")]
    CreateSettingsDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read settings from {path}")]
    ReadSettings {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("settings file at {path} is not valid JSON")]
    ParseSettings {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not write settings to {path}")]
    WriteSettings {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize settings")]
    SerializeSettings(#[source] serde_json::Error),
    #[error("settings contain invalid paths")]
    InvalidSettings(Vec<String>),
}

impl AppError {
    pub fn user_message(&self) -> String {
        match self {
            Self::SettingsDirectory(_) => {
                "ModelHub could not find a safe location for its settings.".to_string()
            }
            Self::CreateSettingsDirectory { path, .. } => format!(
                "ModelHub could not create the settings folder at {}.",
                path.display()
            ),
            Self::ReadSettings { path, .. } => {
                format!("ModelHub could not read settings from {}.", path.display())
            }
            Self::ParseSettings { path, .. } => format!(
                "ModelHub settings at {} are not valid JSON. Fix or remove the file, then try again.",
                path.display()
            ),
            Self::WriteSettings { path, .. } => {
                format!("ModelHub could not save settings to {}.", path.display())
            }
            Self::SerializeSettings(_) => {
                "ModelHub could not prepare settings for saving.".to_string()
            }
            Self::InvalidSettings(messages) => format!(
                "ModelHub could not save settings because {}",
                messages.join(" ")
            ),
        }
    }
}

pub type CommandResult<T> = Result<T, String>;

pub fn into_command_error(error: AppError) -> String {
    error.user_message()
}
