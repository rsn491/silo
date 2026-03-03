use std::collections::HashSet;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::services::agent_launcher::LaunchError;
use crate::services::workspace_kind::WorkspaceKind;

#[derive(Debug, Clone, PartialEq)]
pub struct GitStatus {
    pub kind: WorkspaceKind,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub has_uncommitted_changes: bool,
    pub uncommitted_file_count: usize,
    pub commits_ahead: usize,
    pub commits_behind: usize,
}

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("Git error: {0}")]
    Git(#[from] GitError),
}

pub struct RemovedWorkspace {
    pub path: PathBuf,
    pub kind: WorkspaceKind,
    pub branch: Option<String>,
}

pub struct FailedWorkspace {
    pub path: PathBuf,
    pub error: String,
}

pub struct SkippedWorkspace {
    pub path: PathBuf,
    pub kind: WorkspaceKind,
    pub branch: Option<String>,
    pub commits_ahead: usize,
}

#[derive(Default)]
pub struct CleanupResult {
    pub removed: Vec<RemovedWorkspace>,
    pub failed: Vec<FailedWorkspace>,
    pub skipped: Vec<SkippedWorkspace>,
}

impl CleanupResult {
    pub fn extend(&mut self, other: CleanupResult) {
        self.removed.extend(other.removed);
        self.failed.extend(other.failed);
        self.skipped.extend(other.skipped);
    }
}

#[derive(Debug, Error)]
pub enum CleanupError {
    #[error("Git error: {0}")]
    Git(#[from] GitError),
}

/// Returns `Some(commits_ahead)` when the workspace at `path` has unpushed
/// commits relative to `base_branch`, otherwise `None` (no remote, git error,
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

pub trait WorkspaceFactory {
    /// Creates a new workspace and returns its path.
    ///
    /// # Arguments
    /// * `branch` - Optional branch name for the workspace. If None, a default branch name will be generated.
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError>;
}

pub trait WorkspaceManager {
    /// Removes all inactive workspaces and returns per-item results.
    fn cleanup(
        &self,
        excluded_paths: &HashSet<PathBuf>,
        all: bool,
        force: bool,
    ) -> Result<CleanupResult, CleanupError>;

    /// Returns all workspaces.
    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError>;
}
