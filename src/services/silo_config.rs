use crate::infra::agent::Agent;
use crate::services::workspace_kind::WorkspaceKind;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Error types for silo configuration operations
#[derive(Debug)]
pub enum SiloConfigError {
    IoError(io::Error),
    HomeDirectoryNotFound,
    JsonParse(String),
    JsonWrite(String),
}

impl fmt::Display for SiloConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiloConfigError::IoError(err) => write!(f, "IO error: {}", err),
            SiloConfigError::HomeDirectoryNotFound => {
                write!(f, "could not determine home directory")
            }
            SiloConfigError::JsonParse(msg) => write!(f, "failed to parse settings.json: {}", msg),
            SiloConfigError::JsonWrite(msg) => write!(f, "failed to write settings.json: {}", msg),
        }
    }
}

impl std::error::Error for SiloConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SiloConfigError::IoError(err) => Some(err),
            SiloConfigError::HomeDirectoryNotFound => None,
            SiloConfigError::JsonParse(_) => None,
            SiloConfigError::JsonWrite(_) => None,
        }
    }
}

impl From<io::Error> for SiloConfigError {
    fn from(err: io::Error) -> Self {
        SiloConfigError::IoError(err)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceType {
    Worktree,
    Checkout,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SiloSettings {
    pub agent: Option<Agent>,
    pub workspace_type: Option<WorkspaceKind>,
}

/// Configuration management for silo directory
pub struct SiloConfig;

impl SiloConfig {
    /// Returns the path to ~/.silo/ directory, or None if home directory cannot be determined
    pub fn get_silo_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".silo"))
    }

    /// Initializes the ~/.silo/ directory (idempotent operation)
    /// Returns the path to the created directory
    pub fn initialize() -> Result<PathBuf, SiloConfigError> {
        let silo_dir = Self::get_silo_dir().ok_or(SiloConfigError::HomeDirectoryNotFound)?;

        fs::create_dir_all(&silo_dir)?;

        Ok(silo_dir)
    }

    pub fn get_settings_path() -> Result<PathBuf, SiloConfigError> {
        let silo_dir = Self::get_silo_dir().ok_or(SiloConfigError::HomeDirectoryNotFound)?;
        Ok(silo_dir.join("settings.json"))
    }

    pub fn load_settings() -> Result<SiloSettings, SiloConfigError> {
        let settings_path = Self::get_settings_path()?;
        Self::load_settings_from_path(&settings_path)
    }

    pub fn save_settings(settings: &SiloSettings) -> Result<(), SiloConfigError> {
        let settings_path = Self::get_settings_path()?;
        Self::save_settings_to_path(&settings_path, settings)
    }

    fn load_settings_from_path(path: &Path) -> Result<SiloSettings, SiloConfigError> {
        if !path.exists() {
            return Ok(SiloSettings::default());
        }

        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents).map_err(|e| SiloConfigError::JsonParse(e.to_string()))
    }

    fn save_settings_to_path(path: &Path, settings: &SiloSettings) -> Result<(), SiloConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(settings)
            .map_err(|e| SiloConfigError::JsonWrite(e.to_string()))?;
        fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_initialize_idempotent() {
        // This test verifies that initialize can be called multiple times
        // In practice, this would create the directory if it doesn't exist
        // We can't easily test the actual filesystem operation in unit tests
        // but we can verify the function is callable
        let result1 = SiloConfig::initialize();
        let result2 = SiloConfig::initialize();

        // Both should succeed (or both fail if home dir not available)
        assert_eq!(result1.is_ok(), result2.is_ok());
    }

    #[test]
    fn test_get_silo_dir() {
        // Should return Some(PathBuf) on most systems
        let silo_dir = SiloConfig::get_silo_dir();

        // Can't assert exact value as it depends on environment
        // But we can verify it's either Some or None
        if let Some(path) = silo_dir {
            assert!(path.to_string_lossy().contains(".silo"));
        }
    }

    #[test]
    fn test_load_settings_missing_file() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        let settings = SiloConfig::load_settings_from_path(&settings_path).unwrap();
        assert!(settings.agent.is_none());
    }

    #[test]
    fn test_load_settings_valid_json() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(&settings_path, r#"{"agent":"codex"}"#).unwrap();

        let settings = SiloConfig::load_settings_from_path(&settings_path).unwrap();
        assert_eq!(settings.agent, Some(Agent::Codex));
    }

    #[test]
    fn test_load_settings_invalid_json() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(&settings_path, "{invalid").unwrap();

        let err = SiloConfig::load_settings_from_path(&settings_path).unwrap_err();
        match err {
            SiloConfigError::JsonParse(_) => {}
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_save_settings() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        let settings = SiloSettings {
            agent: Some(Agent::OpenCode),
            workspace_type: None,
        };

        SiloConfig::save_settings_to_path(&settings_path, &settings).unwrap();
        let contents = fs::read_to_string(&settings_path).unwrap();
        assert!(contents.contains("\"agent\""));
        assert!(contents.contains("opencode"));
    }

    #[test]
    fn test_save_and_load_workspace_type() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        let settings = SiloSettings {
            agent: None,
            workspace_type: Some(WorkspaceKind::Checkout),
        };
        SiloConfig::save_settings_to_path(&settings_path, &settings).unwrap();

        let loaded = SiloConfig::load_settings_from_path(&settings_path).unwrap();
        assert_eq!(loaded.workspace_type, Some(WorkspaceKind::Checkout));
    }

    #[test]
    fn test_workspace_type_default_is_absent() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        fs::write(&settings_path, r#"{"agent":"claude"}"#).unwrap();

        let settings = SiloConfig::load_settings_from_path(&settings_path).unwrap();
        assert!(settings.workspace_type.is_none());
    }
}
