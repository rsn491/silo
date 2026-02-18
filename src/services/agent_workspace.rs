use std::path::PathBuf;

use crate::services::{
    agent_launcher::LaunchError,
    git_worktree_workspace::{GitStatus, StatusError},
};

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
