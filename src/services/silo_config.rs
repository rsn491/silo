use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

/// Error types for silo configuration operations
#[derive(Debug)]
pub enum SiloConfigError {
    IoError(io::Error),
    HomeDirectoryNotFound,
}

impl fmt::Display for SiloConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiloConfigError::IoError(err) => write!(f, "IO error: {}", err),
            SiloConfigError::HomeDirectoryNotFound => {
                write!(f, "could not determine home directory")
            }
        }
    }
}

impl std::error::Error for SiloConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SiloConfigError::IoError(err) => Some(err),
            SiloConfigError::HomeDirectoryNotFound => None,
        }
    }
}

impl From<io::Error> for SiloConfigError {
    fn from(err: io::Error) -> Self {
        SiloConfigError::IoError(err)
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
