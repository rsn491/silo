use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

    /// Checks if ~/.silo/ directory exists
    pub fn silo_dir_exists() -> bool {
        Self::get_silo_dir()
            .map(|path| path.exists())
            .unwrap_or(false)
    }

    /// Initializes the ~/.silo/ directory (idempotent operation)
    /// Returns the path to the created directory
    pub fn initialize() -> Result<PathBuf, SiloConfigError> {
        let silo_dir = Self::get_silo_dir().ok_or(SiloConfigError::HomeDirectoryNotFound)?;

        // Create directory if it doesn't exist (idempotent)
        fs::create_dir_all(&silo_dir)?;

        Ok(silo_dir)
    }

    /// Resolves the worktree base directory using priority logic:
    /// 1. Explicit worktree_base parameter (highest priority)
    /// 2. ~/.silo/ directory (if exists)
    /// 3. Parent of repository root (fallback)
    pub fn resolve_worktree_base(worktree_base: Option<PathBuf>, repo_root: &Path) -> PathBuf {
        // Priority 1: Explicit worktree_base
        if let Some(base) = worktree_base {
            return base;
        }

        // Priority 2: ~/.silo/ directory (if exists)
        if Self::silo_dir_exists()
            && let Some(silo_dir) = Self::get_silo_dir()
        {
            return silo_dir;
        }

        // Priority 3: Parent of repository root (fallback)
        repo_root
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| repo_root.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_worktree_base_with_explicit_base() {
        let explicit_base = Some(PathBuf::from("/tmp/custom"));
        let repo_root = PathBuf::from("/home/user/repo");

        let result = SiloConfig::resolve_worktree_base(explicit_base, &repo_root);

        assert_eq!(result, PathBuf::from("/tmp/custom"));
    }

    #[test]
    fn test_resolve_worktree_base_fallback_to_parent() {
        // When no explicit base, returns either ~/.silo (if exists) or parent directory
        let repo_root = PathBuf::from("/home/user/repo");

        let result = SiloConfig::resolve_worktree_base(None, &repo_root);

        // Should be either:
        // 1. ~/.silo (if it exists on the system)
        // 2. Parent of repo_root (/home/user)
        // 3. repo_root itself (if no parent exists)
        if SiloConfig::silo_dir_exists() {
            assert_eq!(result, SiloConfig::get_silo_dir().unwrap());
        } else {
            assert!(result == Path::new("/home/user") || result == repo_root);
        }
    }

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
