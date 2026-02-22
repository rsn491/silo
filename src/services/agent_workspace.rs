use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

use crate::infra::git::GitWorkspaceInfo;
use crate::infra::git_error::GitError;
use crate::services::agent_launcher::LaunchError;

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceKind {
    Worktree,
    Checkout,
}

impl fmt::Display for WorkspaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceKind::Worktree => write!(f, "worktree"),
            WorkspaceKind::Checkout => write!(f, "checkout"),
        }
    }
}

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

#[derive(Default)]
pub struct CleanupResult {
    pub removed: Vec<RemovedWorkspace>,
    pub failed: Vec<FailedWorkspace>,
}

impl CleanupResult {
    pub fn extend(&mut self, other: CleanupResult) {
        self.removed.extend(other.removed);
        self.failed.extend(other.failed);
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
    ) -> Result<CleanupResult, CleanupError>;

    /// Returns all workspaces.
    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError>;
}
