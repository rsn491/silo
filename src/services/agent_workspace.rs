use std::path::PathBuf;

use crate::infra::git_error::GitError;
use crate::services::agent_launcher::LaunchError;

#[derive(Debug, Clone, PartialEq)]
pub struct GitStatus {
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
}
