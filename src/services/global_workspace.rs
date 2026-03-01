use std::collections::HashSet;
use std::path::PathBuf;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::services::agent_workspace::{
    CleanupError, CleanupResult, GitStatus, StatusError, WorkspaceManager,
};
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;

pub struct GlobalWorkspaceManager<G: GitOperations + Clone> {
    worktree_workspaces: GitWorktreeWorkspace<G>,
    checkout_workspaces: GitCheckoutWorkspace<G>,
}

impl<G: GitOperations + Clone> GlobalWorkspaceManager<G> {
    pub fn new(worktree: GitWorktreeWorkspace<G>, checkout: GitCheckoutWorkspace<G>) -> Self {
        Self {
            worktree_workspaces: worktree,
            checkout_workspaces: checkout,
        }
    }

    pub fn with_git(git: G) -> Self {
        let worktree = GitWorktreeWorkspace::new(git.clone());
        let checkout = GitCheckoutWorkspace::new(git);
        Self::new(worktree, checkout)
    }
}

impl<G: GitOperations + Clone> WorkspaceManager for GlobalWorkspaceManager<G> {
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
    use crate::infra::git::GitWorkspaceInfo;
    use crate::infra::git_error::GitError;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    // Mock GitOperations
    #[derive(Clone)]
    struct MockGit {
        repo_root: PathBuf,
        project_name: String,
        worktrees: Vec<GitWorkspaceInfo>,
        base_branch: String,
        uncommitted_changes: Vec<(PathBuf, bool, usize)>,
        divergence: Vec<(PathBuf, usize, usize)>,
    }

    impl GitOperations for MockGit {
        fn get_repo_root(&self) -> Result<PathBuf, GitError> {
            Ok(self.repo_root.clone())
        }

        fn get_project_name(&self) -> Result<String, GitError> {
            Ok(self.project_name.clone())
        }

        fn create_worktree(&self, _path: &Path, _branch: &str) -> Result<(), GitError> {
            Ok(())
        }

        fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
            Ok(self.worktrees.clone())
        }

        fn remove_worktree(&self, _path: &Path) -> Result<(), GitError> {
            Ok(())
        }

        fn get_default_remote_branch(&self) -> Result<String, GitError> {
            Ok(self.base_branch.clone())
        }

        fn get_status_porcelain(&self, worktree_path: &Path) -> Result<String, GitError> {
            for (path, has_changes, count) in &self.uncommitted_changes {
                if path == worktree_path {
                    if *has_changes {
                        let lines: Vec<String> =
                            (0..*count).map(|i| format!(" M file{}.txt", i)).collect();
                        return Ok(lines.join("\n"));
                    } else {
                        return Ok(String::new());
                    }
                }
            }
            Ok(String::new())
        }

        fn count_commits_ahead(
            &self,
            worktree_path: &Path,
            _base_branch: &str,
        ) -> Result<usize, GitError> {
            for (path, ahead, _) in &self.divergence {
                if path == worktree_path {
                    return Ok(*ahead);
                }
            }
            Ok(0)
        }

        fn count_commits_behind(
            &self,
            worktree_path: &Path,
            _base_branch: &str,
        ) -> Result<usize, GitError> {
            for (path, _, behind) in &self.divergence {
                if path == worktree_path {
                    return Ok(*behind);
                }
            }
            Ok(0)
        }

        fn clone_local(&self, _source: &Path, _dest: &Path) -> Result<(), GitError> {
            Ok(())
        }

        fn checkout_new_branch(&self, _path: &Path, _branch: &str) -> Result<(), GitError> {
            Ok(())
        }

        fn get_current_branch(&self, _path: &Path) -> Result<Option<String>, GitError> {
            Ok(None)
        }
    }

    #[test]
    fn test_get_statuses_combines() {
        let repo_root = PathBuf::from("/repo");
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                GitWorkspaceInfo {
                    path: PathBuf::from("/wt1"),
                    branch: Some("branch".to_string()),
                },
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        };
        let worktree = GitWorktreeWorkspace::new(mock_git);

        let mock_git_checkout = MockGit {
            repo_root: PathBuf::from("/repo"),
            project_name: "test-project".to_string(),
            worktrees: vec![],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        };
        let checkout = GitCheckoutWorkspace::new(mock_git_checkout);

        let service = GlobalWorkspaceManager::new(worktree, checkout);

        let statuses = service.get_statuses(true).unwrap();

        assert_eq!(statuses.len(), 1);
        assert!(statuses.iter().any(|s| s.path == PathBuf::from("/wt1")));
    }

    #[test]
    fn test_cleanup_combines() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/repo"),
            project_name: "test-project".to_string(),
            worktrees: vec![],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        };
        let worktree = GitWorktreeWorkspace::new(mock_git);
        let checkout = GitCheckoutWorkspace::new(MockGit {
            repo_root: PathBuf::from("/repo"),
            project_name: "test-project".to_string(),
            worktrees: vec![],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        });
        let service = GlobalWorkspaceManager::new(worktree, checkout);

        let result = service.cleanup(&HashSet::new(), true, false).unwrap();

        assert_eq!(result.removed.len(), 0);
        assert_eq!(result.skipped.len(), 0);
    }

    #[test]
    fn test_get_all_combines() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/repo"),
            project_name: "test-project".to_string(),
            worktrees: vec![GitWorkspaceInfo {
                path: PathBuf::from("/wt1"),
                branch: Some("branch".to_string()),
            }],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        };
        let worktree = GitWorktreeWorkspace::new(mock_git);
        let checkout = GitCheckoutWorkspace::new(MockGit {
            repo_root: PathBuf::from("/repo"),
            project_name: "test-project".to_string(),
            worktrees: vec![],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        });
        let service = GlobalWorkspaceManager::new(worktree, checkout);

        let all = service.get_all().unwrap();

        assert_eq!(all.len(), 1);
        assert!(all.iter().any(|w| w.path == PathBuf::from("/wt1")));
    }
}
