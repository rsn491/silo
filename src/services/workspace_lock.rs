//! Lock-file mechanism for marking a workspace as in use.
//!
//! A `silo.lock` file inside the workspace directory signals that an agent
//! is currently running there.  The lock is acquired atomically; if it
//! already exists **and** the owning process is still alive, acquisition
//! fails.
//!
//! The lock file contains the PID of the owning process as a decimal string.
//! On acquire, if a lock file already exists but its PID is no longer alive
//! (i.e. the previous agent crashed), the stale lock is silently reclaimed.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Name of the lock file placed inside each workspace directory.
const LOCK_FILE_NAME: &str = "silo.lock";

/// Errors that can occur when managing a workspace lock.
#[derive(Debug, Error)]
pub enum LockError {
    /// The workspace is already locked by a live agent process.
    #[error(
        "workspace is already locked by PID {pid}: {path}\nRemove the lock file to continue: rm {path}"
    )]
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
    pub fn try_acquire(&self) -> Result<(), LockError> {
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.path)
            {
                Ok(mut file) => {
                    // Write our PID so future acquirers can detect stale locks.
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(());
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

    /// Releases the lock by deleting the lock file.
    ///
    /// Errors are silently ignored — if the file was already removed the
    /// workspace is effectively unlocked regardless.
    pub fn release(&self) {
        let _ = std::fs::remove_file(&self.path);
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
        .and_then(|s| s.trim().parse().ok())
}

/// Returns `true` if the process with `pid` is currently running.
///
/// On Linux this is done by checking for the existence of `/proc/<pid>`,
/// which is reliable and requires no external crates.  On other Unix
/// systems the check falls back to a conservative `true` (assume alive)
/// so that we never incorrectly reclaim a live lock on an unsupported
/// platform.
fn pid_is_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        // Conservative fallback: assume the process is alive.
        true
    }
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
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());

        // Act
        lock.try_acquire()
            .expect("should acquire lock on fresh directory");

        // Assert
        assert!(dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn lock_file_contains_current_pid() {
        let dir = tmp();
        WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        let contents =
            fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).expect("failed to read lock file");
        let stored_pid: u32 = contents.trim().parse().expect("lock file should contain a PID");

        assert_eq!(stored_pid, std::process::id());
    }

    #[test]
    fn is_locked_reflects_live_process() {
        let dir = tmp();
        assert!(!WorkspaceLock::is_locked(dir.path()));

        // Act
        WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        assert!(WorkspaceLock::is_locked(dir.path()));
    }

    #[test]
    fn acquire_fails_when_already_locked_by_live_pid() {
        let dir = tmp();
        WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should acquire lock on fresh directory");

        let err = WorkspaceLock::new(dir.path()).try_acquire().unwrap_err();

        assert!(matches!(err, LockError::AlreadyLocked { .. }));
    }

    #[test]
    fn release_removes_lock_file() {
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());
        lock.try_acquire()
            .expect("should acquire lock on fresh directory");

        lock.release();

        // Assert
        assert!(!dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn release_is_idempotent() {
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());

        lock.release(); // no lock file — should not panic
        lock.release();

        // Assert
        assert!(!dir.path().join(LOCK_FILE_NAME).exists());
    }

    #[test]
    fn acquire_after_release_succeeds() {
        let dir = tmp();
        let lock = WorkspaceLock::new(dir.path());
        lock.try_acquire()
            .expect("should acquire lock on fresh directory");
        lock.release();

        assert!(lock.try_acquire().is_ok());
    }

    /// Simulate a crashed agent: write a lock file with a PID that is
    /// guaranteed not to exist (PID 0 is the idle/swapper process and is
    /// never a valid user-space PID on Linux).
    #[test]
    #[cfg(target_os = "linux")]
    fn stale_lock_is_reclaimed_on_acquire() {
        let dir = tmp();
        // Write a lock file with a dead PID.
        fs::write(dir.path().join(LOCK_FILE_NAME), b"0")
            .expect("failed to write lock file");

        // Should succeed by reclaiming the stale lock.
        WorkspaceLock::new(dir.path())
            .try_acquire()
            .expect("should reclaim stale lock");

        // The lock file now holds our PID.
        let contents =
            fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).expect("failed to read lock file");
        let stored_pid: u32 = contents.trim().parse().expect("lock file should contain a PID");
        assert_eq!(stored_pid, std::process::id());
    }

    /// A stale lock should not be reported as locked by `is_locked`.
    #[test]
    #[cfg(target_os = "linux")]
    fn stale_lock_is_not_reported_as_locked() {
        let dir = tmp();
        fs::write(dir.path().join(LOCK_FILE_NAME), b"0")
            .expect("failed to write lock file");

        assert!(!WorkspaceLock::is_locked(dir.path()));
    }

    /// A lock file with an unreadable/garbage PID is treated conservatively
    /// as locked (don't accidentally stomp on something we can't identify).
    #[test]
    fn malformed_lock_file_is_treated_as_locked() {
        let dir = tmp();
        fs::write(dir.path().join(LOCK_FILE_NAME), b"not-a-pid")
            .expect("failed to write lock file");

        assert!(WorkspaceLock::is_locked(dir.path()));
    }
}
