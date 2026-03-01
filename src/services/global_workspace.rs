use std::collections::HashSet;
use std::path::PathBuf;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::services::agent_workspace::{
    CleanupError, CleanupResult, GitStatus, StatusError, WorkspaceManager,
};
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;

pub struct GlobalWorkspaceManager<G: GitOperations> {
    worktree_workspaces: GitWorktreeWorkspace<G>,
    checkout_workspaces: GitCheckoutWorkspace<G>,
}

impl<G: GitOperations> GlobalWorkspaceManager<G> {
    pub fn new(worktree: GitWorktreeWorkspace<G>, checkout: GitCheckoutWorkspace<G>) -> Self {
        Self {
            worktree_workspaces: worktree,
            checkout_workspaces: checkout,
        }
    }
}

impl<G: GitOperations + Clone> GlobalWorkspaceManager<G> {
    pub fn with_git(git: G) -> Self {
        let worktree = GitWorktreeWorkspace::new(git.clone());
        let checkout = GitCheckoutWorkspace::new(git);
        Self::new(worktree, checkout)
    }
}

impl<G: GitOperations> WorkspaceManager for GlobalWorkspaceManager<G> {
    fn get_statuses(&self, show_all: bool) -> Result<Vec<GitStatus>, StatusError> {
        Ok([
            self.worktree_workspaces.get_statuses(show_all)?,
            self.checkout_workspaces.get_statuses(show_all)?,
        ]
        .concat())
    }

    fn cleanup(
        &self,
        excluded_paths: &HashSet<PathBuf>,
        all: bool,
        force: bool,
    ) -> Result<CleanupResult, CleanupError> {
        Ok([
            self.worktree_workspaces
                .cleanup(excluded_paths, all, force)?,
            self.checkout_workspaces
                .cleanup(excluded_paths, all, force)?,
        ]
        .into_iter()
        .reduce(|mut acc, item| {
            acc.extend(item);
            acc
        })
        .unwrap_or_default())
    }

    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        Ok([
            self.worktree_workspaces.get_all()?,
            self.checkout_workspaces.get_all()?,
        ]
        .concat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::{GitWorkspaceInfo, MockGitOperations};
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn test_get_statuses_combines() {
        let repo_root = PathBuf::from("/repo");
        let wt1_path = PathBuf::from("/wt1");

        let worktrees = vec![
            GitWorkspaceInfo {
                path: repo_root.clone(),
                branch: Some("main".to_string()),
            },
            GitWorkspaceInfo {
                path: wt1_path.clone(),
                branch: Some("branch".to_string()),
            },
        ];

        let mut worktree_mock = MockGitOperations::new();
        worktree_mock
            .expect_list_worktrees()
            .return_once(move || Ok(worktrees));
        worktree_mock
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        worktree_mock
            .expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        worktree_mock
            .expect_count_commits_ahead()
            .returning(|_, _| Ok(0));
        worktree_mock
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        checkout_mock
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));

        let worktree = GitWorktreeWorkspace::new(worktree_mock);
        let checkout = GitCheckoutWorkspace::new(checkout_mock);
        let service = GlobalWorkspaceManager::new(worktree, checkout);

        let statuses = service.get_statuses(true).unwrap();

        assert_eq!(statuses.len(), 1);
        assert!(statuses.iter().any(|s| s.path == wt1_path));
    }

    #[test]
    fn test_cleanup_combines() {
        let repo_root = PathBuf::from("/repo");

        let mut worktree_mock = MockGitOperations::new();
        worktree_mock
            .expect_list_worktrees()
            .return_once(|| Ok(vec![]));
        worktree_mock
            .expect_get_repo_root()
            .returning(move || Ok(repo_root.clone()));
        worktree_mock
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));

        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        checkout_mock
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));

        let worktree = GitWorktreeWorkspace::new(worktree_mock);
        let checkout = GitCheckoutWorkspace::new(checkout_mock);
        let service = GlobalWorkspaceManager::new(worktree, checkout);

        let result = service.cleanup(&HashSet::new(), true, false).unwrap();

        assert_eq!(result.removed.len(), 0);
        assert_eq!(result.skipped.len(), 0);
    }

    #[test]
    fn test_get_all_combines() {
        let wt1_path = PathBuf::from("/wt1");

        let mut worktree_mock = MockGitOperations::new();
        worktree_mock
            .expect_list_worktrees()
            .return_once(move || {
                Ok(vec![GitWorkspaceInfo {
                    path: wt1_path.clone(),
                    branch: Some("branch".to_string()),
                }])
            });

        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));

        let worktree = GitWorktreeWorkspace::new(worktree_mock);
        let checkout = GitCheckoutWorkspace::new(checkout_mock);
        let service = GlobalWorkspaceManager::new(worktree, checkout);

        let all = service.get_all().unwrap();

        assert_eq!(all.len(), 1);
        assert!(all.iter().any(|w| w.path == PathBuf::from("/wt1")));
    }
}
