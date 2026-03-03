//! Traits and types for managing agent workspaces.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::services::agent_launcher::LaunchError;
use crate::services::workspace_kind::WorkspaceKind;

/// Detailed Git status for a workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct GitStatus {
    /// The kind of workspace (worktree or clone).
    pub kind: WorkspaceKind,
    /// The absolute path to the workspace.
    pub path: PathBuf,
    /// The name of the current branch, if any.
    pub branch: Option<String>,
    /// Whether there are any uncommitted changes.
    pub has_uncommitted_changes: bool,
    /// The number of files with uncommitted changes.
    pub uncommitted_file_count: usize,
    /// Number of commits ahead of the remote base branch.
    pub commits_ahead: usize,
    /// Number of commits behind the remote base branch.
    pub commits_behind: usize,
}

/// Errors that can occur when retrieving workspace status.
#[derive(Debug, Error)]
pub enum StatusError {
    /// A Git operation failed.
    #[error("Git error: {0}")]
    Git(#[from] GitError),
}

/// Details of a workspace that was successfully removed during cleanup.
pub struct RemovedWorkspace {
    /// The path of the removed workspace.
    pub path: PathBuf,
    /// The kind of workspace that was removed.
    pub kind: WorkspaceKind,
    /// The branch that was associated with the workspace.
    pub branch: Option<String>,
}

/// Details of a workspace that failed to be removed during cleanup.
pub struct FailedWorkspace {
    /// The path of the workspace that failed to be removed.
    pub path: PathBuf,
    /// The error message describing the failure.
    pub error: String,
}

/// Details of a workspace that was skipped during cleanup.
pub struct SkippedWorkspace {
    /// The path of the skipped workspace.
    pub path: PathBuf,
    /// The kind of workspace that was skipped.
    pub kind: WorkspaceKind,
    /// The branch associated with the workspace.
    pub branch: Option<String>,
    /// Number of unpushed commits that caused the workspace to be skipped.
    pub commits_ahead: usize,
}

/// Summary of a workspace cleanup operation.
#[derive(Default)]
pub struct CleanupResult {
    /// List of successfully removed workspaces.
    pub removed: Vec<RemovedWorkspace>,
    /// List of workspaces that failed to be removed.
    pub failed: Vec<FailedWorkspace>,
    /// List of workspaces that were skipped (e.g., due to unpushed commits).
    pub skipped: Vec<SkippedWorkspace>,
}

impl CleanupResult {
    /// Merges another cleanup result into this one.
    pub fn extend(&mut self, other: CleanupResult) {
        self.removed.extend(other.removed);
        self.failed.extend(other.failed);
        self.skipped.extend(other.skipped);
    }
}

/// Errors that can occur during workspace cleanup.
#[derive(Debug, Error)]
pub enum CleanupError {
    /// A Git operation failed.
    #[error("Git error: {0}")]
    Git(#[from] GitError),
}

/// Returns the number of commits ahead of the remote base branch.
///
/// Returns `Some(commits_ahead)` when the workspace at `path` has unpushed
/// commits relative to `base_branch`, otherwise `None` (no remote, Git error,
/// or zero commits ahead).
pub fn commits_ahead_of_remote<G: GitOperations>(
    git: &G,
    path: &Path,
    base_branch: &Option<String>,
) -> Option<usize> {
    let base = base_branch.as_deref()?;
    let ahead = git.count_commits_ahead(path, base).ok()?;
    if ahead > 0 { Some(ahead) } else { None }
}

/// Trait for creating new workspaces.
pub trait WorkspaceFactory {
    /// Creates a new workspace and returns its path.
    ///
    /// # Arguments
    ///
    /// * `branch` - Optional branch name for the workspace. If `None`, a default branch name will be generated.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if workspace creation fails.
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError>;
}

/// Trait for managing and inspecting existing workspaces.
pub trait WorkspaceManager {
    /// Removes all inactive workspaces and returns per-item results.
    ///
    /// # Errors
    ///
    /// Returns [`CleanupError`] if the cleanup operation fails.
    fn cleanup(
        &self,
        excluded_paths: &HashSet<PathBuf>,
        all: bool,
        force: bool,
    ) -> Result<CleanupResult, CleanupError>;

    /// Returns all workspaces.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if listing workspaces fails.
    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError>;
}
