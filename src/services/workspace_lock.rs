//! Lock-file mechanism for marking a workspace as in use.
//!
//! A `silo.lock` file inside the workspace directory signals that an agent
//! is currently running there.  The lock is acquired atomically; if it
//! already exists the acquisition fails.
//!
//! Stale locks (e.g. after a crash or an async tab/split-pane launch) must
//! be removed manually:
//!
//! ```text
//! rm <workspace>/silo.lock
//! ```

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Name of the lock file placed inside each workspace directory.
const LOCK_FILE_NAME: &str = "silo.lock";

/// Errors that can occur when managing a workspace lock.
#[derive(Debug, Error)]
pub enum LockError {
    /// The workspace is already locked by another agent.
    #[error(
        "workspace is already locked (stale lock?): {0}\nRemove the lock file to continue: rm {0}"
    )]
    AlreadyLocked(PathBuf),
    /// An I/O error occurred while creating or removing the lock file.
    #[error("I/O error while managing workspace lock: {0}")]
    Io(#[from] std::io::Error),
}

/// A lock tied to a single workspace directory.
pub struct WorkspaceLock {
    /// Absolute path to the `silo.lock` file inside the workspace.
    path: PathBuf,
}

impl WorkspaceLock {
    /// Creates a `WorkspaceLock` for the given workspace directory.
    ///
    /// The lock file path will be `workspace_path/silo.lock`.
    pub fn new(workspace_path: &Path) -> Self {
        Self {
            path: workspace_path.join(LOCK_FILE_NAME),
        }
    }

    /// Atomically acquires the lock by creating the lock file.
    ///
    /// Fails with [`LockError::AlreadyLocked`] if the lock file already
    /// exists (i.e. another agent holds the workspace).
    ///
    /// # Errors
    ///
    /// Returns [`LockError`] if the lock cannot be acquired.
    pub fn try_acquire(&self) -> Result<(), LockError> {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(LockError::AlreadyLocked(self.path.clone()))
            }
            Err(e) => Err(LockError::Io(e)),
        }
    }

    /// Releases the lock by deleting the lock file.
    ///
    /// If the file was already removed the workspace is effectively unlocked
    /// regardless, but unexpected I/O errors are logged as warnings.
    pub fn release(&self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[warn] failed to release workspace lock {:?}: {}", self.path, e);
            }
        }
    }

    /// Returns `true` when `silo.lock` exists inside `workspace_path`.
    pub fn is_locked(workspace_path: &Path) -> bool {
        workspace_path.join(LOCK_FILE_NAME).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    #[test]
    fn acquire_creates_lock_file() {
        // Arrange
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());

        // Act
        lock.try_acquire()
            .expect("should acquire lock on fresh directory");

        // Assert
        assert!(dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn is_locked_reflects_file_presence() {
        // Arrange
        let dir = tmp();
        assert!(!WorkspaceLock::is_locked(dir.path()));

        // Act
        WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Assert
        assert!(WorkspaceLock::is_locked(dir.path()));
    }

    #[test]
    fn acquire_fails_when_already_locked() {
        // Arrange
        let dir = tmp();
        WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Act
        let err = WorkspaceLock::new(dir.path()).try_acquire().unwrap_err();

        // Assert
        assert!(matches!(err, LockError::AlreadyLocked(_)));
    }

    #[test]
    fn release_removes_lock_file() {
        // Arrange
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());
        lock.try_acquire()
            .expect("should acquire lock on fresh directory");

        // Act
        lock.release();

        // Assert
        assert!(!dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn release_is_idempotent() {
        // Arrange
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());

        // Act
        lock.release(); // no lock file — should not panic
        lock.release();

        // Assert
        assert!(!dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn acquire_after_release_succeeds() {
        // Arrange
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());
        lock.try_acquire()
            .expect("should acquire lock on fresh directory");
        lock.release();

        // Act
        let result = lock.try_acquire();

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn manually_placed_lock_file_is_detected() {
        // Arrange
        let dir = tmp();
        fs::write(dir.path().join(LOCK_FILE_NAME), b"").expect("failed to write lock file");

        // Act
        let is_locked = WorkspaceLock::is_locked(dir.path());
        let err = WorkspaceLock::new(dir.path()).try_acquire().unwrap_err();

        // Assert
        assert!(is_locked);
        assert!(matches!(err, LockError::AlreadyLocked(_)));
    }
}
