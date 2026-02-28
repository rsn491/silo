use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

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

#[derive(Debug)]
pub enum StatusError {
    Git(GitError),
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusError::Git(e) => write!(f, "Git error: {}", e),
        }
    }
}

impl std::error::Error for StatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StatusError::Git(e) => Some(e),
        }
    }
}

impl From<GitError> for StatusError {
    fn from(error: GitError) -> Self {
        StatusError::Git(error)
    }
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

#[derive(Debug)]
pub enum CleanupError {
    Git(GitError),
    Io(String),
}

impl fmt::Display for CleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CleanupError::Git(e) => write!(f, "Git error: {}", e),
            CleanupError::Io(s) => write!(f, "IO error: {}", s),
        }
    }
}

impl std::error::Error for CleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CleanupError::Git(e) => Some(e),
            CleanupError::Io(_) => None,
        }
    }
}

impl From<GitError> for CleanupError {
    fn from(error: GitError) -> Self {
        CleanupError::Git(error)
    }
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

pub trait AgentWorkspaceManager {
    /// Creates a new workspace and returns its path.
    ///
    /// # Arguments
    /// * `branch` - Optional branch name for the workspace. If None, a default branch name will be generated.
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError>;

    /// Returns the git status of each workspace.
    ///
    /// # Arguments
    /// * `show_all` - If true, returns git status for each workspace, including clean workspaces.
    fn get_statuses(&self, show_all: bool) -> Result<Vec<GitStatus>, StatusError>;

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
