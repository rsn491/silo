//! Lock-file mechanism for serializing launches into a workspace.
//!
//! A `silo.lock` file inside the workspace directory signals that an agent
//! launch is currently in progress there. The lock is acquired atomically and
//! released automatically when its guard is dropped.
//!
//! The lock file contains the PID and a process-local acquisition identifier.
//! On acquire, if a lock file already exists but its PID is no longer alive
//! (i.e. the previous agent crashed), the stale lock is silently reclaimed.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use crate::infra::system_process::pid_is_alive;

/// Name of the lock file placed inside each workspace directory.
const LOCK_FILE_NAME: &str = "silo.lock";

/// Generates distinct ownership identifiers for acquisitions in this process.
static NEXT_LOCK_ID: AtomicU64 = AtomicU64::new(1);

/// Errors that can occur when managing a workspace lock.
#[derive(Debug, Error)]
pub enum LockError {
    /// The workspace already has a launch in progress.
    #[error("workspace launch is already in progress under PID {pid}: {path}")]
    AlreadyLocked {
        /// Path to the lock file.
        path: PathBuf,
        /// PID of the process holding the lock.
        pid: u32,
    },
    /// An I/O error occurred while creating or removing the lock file.
    #[error("I/O error while managing workspace lock: {0}")]
    Io(#[from] std::io::Error),
}

/// A lock tied to a single workspace directory.
pub struct WorkspaceLock {
    /// Absolute path to the `silo.lock` file inside the workspace.
    path: PathBuf,
}

/// Ownership guard for an acquired workspace launch lock.
///
/// Only successful acquisition can create this guard, so a failed contender
/// can never remove another launcher's lock. The file is removed on drop.
#[must_use = "the lock is released when its guard is dropped"]
#[derive(Debug)]
pub struct WorkspaceLockGuard {
    /// Path owned by this successful acquisition.
    path: PathBuf,
    /// Exact contents written by this acquisition.
    owner: String,
}

impl Drop for WorkspaceLockGuard {
    fn drop(&mut self) {
        if std::fs::read_to_string(&self.path).ok().as_deref() != Some(self.owner.as_str()) {
            return;
        }
        if let Err(e) = std::fs::remove_file(&self.path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("failed to release workspace lock {:?}: {}", self.path, e);
        }
    }
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

    /// Atomically acquires the lock by creating the lock file with the
    /// current process PID.
    ///
    /// If a lock file already exists but its recorded PID is no longer
    /// running (stale lock from a crashed agent), the stale file is removed
    /// and the lock is re-acquired for the current process.
    ///
    /// Fails with [`LockError::AlreadyLocked`] only when the lock file
    /// exists **and** its PID corresponds to a live process.
    ///
    /// # Errors
    ///
    /// Returns [`LockError`] if the lock cannot be acquired.
    pub fn try_acquire(&self) -> Result<WorkspaceLockGuard, LockError> {
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.path)
            {
                Ok(mut file) => {
                    let owner = format!(
                        "{}:{}",
                        std::process::id(),
                        NEXT_LOCK_ID.fetch_add(1, Ordering::Relaxed)
                    );
                    // Record the PID for stale detection and a unique identifier
                    // so this guard only removes the lock it acquired.
                    if let Err(e) = write!(file, "{owner}") {
                        drop(file);
                        let _ = std::fs::remove_file(&self.path);
                        return Err(LockError::Io(e));
                    }
                    return Ok(WorkspaceLockGuard {
                        path: self.path.clone(),
                        owner,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    match read_pid_from_lock(&self.path) {
                        Some(pid) if pid_is_alive(pid) => {
                            return Err(LockError::AlreadyLocked {
                                path: self.path.clone(),
                                pid,
                            });
                        }
                        _ => {
                            // PID is dead or unreadable — stale lock; reclaim it.
                            // Ignore the error: if removal fails we'll either
                            // succeed on the next create_new or hit AlreadyExists
                            // again and re-evaluate.
                            let _ = std::fs::remove_file(&self.path);
                            // Loop back and retry the atomic create.
                        }
                    }
                }
                Err(e) => return Err(LockError::Io(e)),
            }
        }
    }

    /// Returns `true` when `silo.lock` exists inside `workspace_path` **and**
    /// the PID recorded in it belongs to a live process.
    ///
    /// A lock file whose PID is no longer running is treated as unlocked (the
    /// file will be reclaimed on the next [`WorkspaceLock::try_acquire`]).
    pub fn is_locked(workspace_path: &Path) -> bool {
        let path = workspace_path.join(LOCK_FILE_NAME);
        if !path.exists() {
            return false;
        }
        match read_pid_from_lock(&path) {
            Some(pid) => pid_is_alive(pid),
            // Unreadable / malformed lock file — treat conservatively as locked.
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read the PID stored in a lock file, returning `None` if the file is
/// missing or its contents cannot be parsed as a `u32`.
fn read_pid_from_lock(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().split(':').next()?.parse().ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        let _guard = lock
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Assert
        assert!(dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn lock_file_contains_current_pid() {
        // Arrange
        let dir = tmp();
        let _guard = WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Act
        let stored_pid = read_pid_from_lock(&dir.path().join(LOCK_FILE_NAME))
            .expect("lock file should contain a PID");

        // Assert
        assert_eq!(stored_pid, std::process::id());
    }

    #[test]
    fn is_locked_reflects_live_process() {
        // Arrange
        let dir = tmp();
        assert!(!WorkspaceLock::is_locked(dir.path()));

        // Act
        let _guard = WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Assert
        assert!(WorkspaceLock::is_locked(dir.path()));
    }

    #[test]
    fn acquire_fails_when_already_locked_by_live_pid() {
        // Arrange
        let dir = tmp();
        let _guard = WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Act
        let err = WorkspaceLock::new(dir.path()).try_acquire().unwrap_err();

        // Assert
        assert!(matches!(err, LockError::AlreadyLocked { .. }));
        assert!(dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn release_removes_lock_file() {
        // Arrange
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());
        let guard = lock
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        // Act
        drop(guard);

        // Assert
        assert!(!dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn acquire_after_release_succeeds() {
        // Arrange
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());
        let guard = lock
            .try_acquire()
            .expect("should acquire lock on fresh directory");
        drop(guard);

        // Act
        let _guard = lock.try_acquire().expect("should reacquire released lock");

        // Assert
        assert!(dir.path().join(LOCK_FILE_NAME).exists());
    }

    /// Simulate a crashed agent: write a lock file with a PID that is
    /// guaranteed not to exist (PID 0 is the idle/swapper process and is
    /// never a valid user-space PID on Linux).
    #[test]
    #[cfg(target_os = "linux")]
    fn stale_lock_is_reclaimed_on_acquire() {
        // Arrange
        let dir = tmp();
        // Write a lock file with a dead PID.
        fs::write(dir.path().join(LOCK_FILE_NAME), b"0").expect("failed to write lock file");

        // Act
        let _guard = WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should reclaim stale lock");

        // Assert
        let stored_pid = read_pid_from_lock(&dir.path().join(LOCK_FILE_NAME))
            .expect("lock file should contain a PID");
        assert_eq!(stored_pid, std::process::id());
    }

    /// A stale lock should not be reported as locked by `is_locked`.
    #[test]
    #[cfg(target_os = "linux")]
    fn stale_lock_is_not_reported_as_locked() {
        // Arrange
        let dir = tmp();
        fs::write(dir.path().join(LOCK_FILE_NAME), b"0").expect("failed to write lock file");

        // Act / Assert
        assert!(!WorkspaceLock::is_locked(dir.path()));
    }

    /// A lock file with an unreadable/garbage PID is treated conservatively
    /// as locked (don't accidentally stomp on something we can't identify).
    #[test]
    fn malformed_lock_file_is_treated_as_locked() {
        // Arrange
        let dir = tmp();
        fs::write(dir.path().join(LOCK_FILE_NAME), b"not-a-pid")
            .expect("failed to write lock file");

        // Act / Assert
        assert!(WorkspaceLock::is_locked(dir.path()));
    }

    #[test]
    fn guard_does_not_remove_a_replacement_lock() {
        let dir = tmp();
        let guard = WorkspaceLock::new(dir.path()).try_acquire().unwrap();
        let path = dir.path().join(LOCK_FILE_NAME);
        fs::write(&path, "replacement").unwrap();

        drop(guard);

        assert_eq!(fs::read_to_string(path).unwrap(), "replacement");
    }
}
