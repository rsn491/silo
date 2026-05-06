//! Traits and types for managing agent workspaces.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::services::agent_launcher::LaunchError;
use crate::services::git_branch_service::UUID_SUFFIX_LEN;
use crate::services::workspace_kind::WorkspaceKind;
use crate::services::workspace_lock::WorkspaceLock;
use uuid::Uuid;

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

/// Attempts to reuse an inactive workspace by checking out a new branch.
///
/// Returns `Ok(Some(path))` when a reusable workspace is found and updated,
/// `Ok(None)` when no suitable workspace exists, or a `LaunchError` when
/// Git operations fail.
pub fn reuse_inactive_workspace<G: GitOperations>(
    git: &G,
    workspaces: Vec<GitWorkspaceInfo>,
    branch: Option<String>,
) -> Result<Option<PathBuf>, LaunchError> {
    let inactive = workspaces.into_iter().find(|ws| {
        !WorkspaceLock::is_locked(&ws.path)
            && !ws.has_uncommitted_changes
            && (ws.commits_ahead == 0 || git.is_fully_pushed(&ws.path))
    });

    if let Some(ws) = inactive {
        let branch_name = branch.unwrap_or_else(|| {
            let project = git
                .get_project_name()
                .unwrap_or_else(|_| "workspace".to_string());
            format!(
                "{}-{}",
                project,
                &Uuid::new_v4().to_string()[..UUID_SUFFIX_LEN]
            )
        });
        git.fetch_remote(&ws.path).map_err(LaunchError::Git)?;
        let default_branch = git.get_default_remote_branch().map_err(LaunchError::Git)?;
        git.checkout_new_branch_from(&ws.path, &branch_name, &default_branch)
            .map_err(LaunchError::Git)?;
        return Ok(Some(ws.path));
    }

    Ok(None)
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
    fn create(&self, branch: Option<String>, reuse: bool) -> Result<PathBuf, LaunchError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::MockGitOperations;
    use tempfile::TempDir;

    fn make_workspace(tmp: &TempDir, commits_ahead: usize) -> GitWorkspaceInfo {
        GitWorkspaceInfo {
            path: tmp.path().to_path_buf(),
            branch: Some("feature".to_string()),
            has_uncommitted_changes: false,
            commits_ahead,
            latest_commit: None,
        }
    }

    #[test]
    fn reuse_picks_workspace_at_base_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = make_workspace(&tmp, 0);

        let mut mock = MockGitOperations::new();
        mock.expect_is_fully_pushed().never();
        mock.expect_get_project_name()
            .returning(|| Ok("proj".to_string()));
        mock.expect_fetch_remote().returning(|_| Ok(()));
        mock.expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock.expect_checkout_new_branch_from()
            .returning(|_, _, _| Ok(()));

        let result = reuse_inactive_workspace(&mock, vec![ws], Some("new-branch".to_string()));
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn reuse_picks_workspace_with_pushed_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = make_workspace(&tmp, 3);

        let mut mock = MockGitOperations::new();
        mock.expect_is_fully_pushed().once().returning(|_| true);
        mock.expect_get_project_name()
            .returning(|| Ok("proj".to_string()));
        mock.expect_fetch_remote().returning(|_| Ok(()));
        mock.expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock.expect_checkout_new_branch_from()
            .returning(|_, _, _| Ok(()));

        let result = reuse_inactive_workspace(&mock, vec![ws], Some("new-branch".to_string()));
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn reuse_skips_workspace_with_unpushed_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = make_workspace(&tmp, 2);

        let mut mock = MockGitOperations::new();
        mock.expect_is_fully_pushed().once().returning(|_| false);
        mock.expect_get_project_name().never();

        let result = reuse_inactive_workspace(&mock, vec![ws], Some("new-branch".to_string()));
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn reuse_skips_locked_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let lock = WorkspaceLock::new(tmp.path());
        lock.try_acquire().unwrap();

        let ws = make_workspace(&tmp, 0);
        let mock = MockGitOperations::new();

        let result = reuse_inactive_workspace(&mock, vec![ws], Some("new-branch".to_string()));
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn reuse_skips_workspace_with_uncommitted_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = GitWorkspaceInfo {
            path: tmp.path().to_path_buf(),
            branch: Some("feature".to_string()),
            has_uncommitted_changes: true,
            commits_ahead: 0,
            latest_commit: None,
        };

        let mock = MockGitOperations::new();

        let result = reuse_inactive_workspace(&mock, vec![ws], Some("new-branch".to_string()));
        assert!(result.unwrap().is_none());
    }
}

/// Finds a workspace whose current branch matches `branch`.
///
/// Returns `None` if no workspace with that branch exists or if listing workspaces fails.
pub fn find_workspace_by_branch<W: WorkspaceManager>(manager: &W, branch: &str) -> Option<PathBuf> {
    manager
        .get_all()
        .ok()?
        .into_iter()
        .find(|ws| ws.branch.as_deref() == Some(branch))
        .map(|ws| ws.path)
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
