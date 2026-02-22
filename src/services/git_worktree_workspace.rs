use std::collections::HashSet;
use std::path::PathBuf;

use uuid::Uuid;

use super::agent_launcher::LaunchError;
use super::agent_workspace::{
    AgentWorkspaceManager, CleanupError, CleanupResult, FailedWorkspace, GitStatus,
    RemovedWorkspace, StatusError, WorkspaceKind,
};
use super::silo_config::SiloConfig;
use crate::infra::git::{GitOperations, GitWorkspaceInfo};

pub struct GitWorktreeWorkspace<G: GitOperations> {
    git: G,
    worktree_base: Option<PathBuf>,
}

impl<G: GitOperations> GitWorktreeWorkspace<G> {
    pub fn new(git: G, worktree_base: Option<PathBuf>) -> Self {
        Self { git, worktree_base }
    }

    fn generate_worktree_path(&self) -> Result<PathBuf, LaunchError> {
        let repo_root = self.git.get_repo_root()?;

        // Use SiloConfig for resolution with priority logic
        let base_dir = SiloConfig::resolve_worktree_base(self.worktree_base.clone(), &repo_root);

        let worktree_name = format!(
            "{}-{}",
            self.git.get_project_name()?,
            &Uuid::new_v4().to_string()[..8]
        );
        Ok(base_dir.join(&worktree_name))
    }

    fn parse_uncommitted_changes(status_output: &str) -> (bool, usize) {
        let file_count = status_output
            .lines()
            .filter(|line| !line.is_empty())
            .count();
        let has_changes = file_count > 0;
        (has_changes, file_count)
    }

    pub fn get_git_status(&self, worktree: GitWorkspaceInfo) -> Result<GitStatus, StatusError> {
        let base_branch = self.git.get_default_remote_branch()?;
        let status_output = self.git.get_status_porcelain(&worktree.path)?;
        let (has_uncommitted_changes, uncommitted_file_count) =
            Self::parse_uncommitted_changes(&status_output);

        let commits_ahead = self.git.count_commits_ahead(&worktree.path, &base_branch)?;
        let commits_behind = self
            .git
            .count_commits_behind(&worktree.path, &base_branch)?;

        Ok(GitStatus {
            kind: WorkspaceKind::Worktree,
            path: worktree.path.clone(),
            branch: worktree.branch.clone(),
            has_uncommitted_changes,
            uncommitted_file_count,
            commits_ahead,
            commits_behind,
        })
    }
}

impl<G: GitOperations> AgentWorkspaceManager for GitWorktreeWorkspace<G> {
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        let worktree_path = self.generate_worktree_path()?;
        let worktree_name = worktree_path.file_name().unwrap().to_string_lossy();
        let branch_name = branch.unwrap_or_else(|| worktree_name.to_string());

        self.git.create_worktree(&worktree_path, &branch_name)?;

        Ok(worktree_path)
    }

    fn get_statuses(&self, show_all: bool) -> Result<Vec<GitStatus>, StatusError> {
        let worktrees = self.git.list_worktrees()?;
        let mut statuses = Vec::new();

        for (index, worktree) in worktrees.iter().enumerate() {
            if index == 0 {
                continue;
            }

            let status = self.get_git_status(worktree.clone())?;

            if show_all
                || status.has_uncommitted_changes
                || status.commits_ahead > 0
                || status.commits_behind > 0
            {
                statuses.push(status);
            }
        }

        Ok(statuses)
    }

    fn cleanup(
        &self,
        active_paths: &HashSet<PathBuf>,
        all: bool,
    ) -> Result<CleanupResult, CleanupError> {
        let all_worktrees = self.git.list_worktrees()?;
        let repo_root = self.git.get_repo_root()?;
        let silo_dir = SiloConfig::get_silo_dir();

        let candidates: Vec<_> = all_worktrees
            .iter()
            .filter(|wt| {
                if wt.path == repo_root {
                    return false;
                }
                if active_paths.contains(&wt.path) {
                    return false;
                }
                if !all {
                    if let Some(ref silo) = silo_dir {
                        return wt.path.starts_with(silo);
                    }
                    return false;
                }
                true
            })
            .collect();

        let mut result = CleanupResult::default();

        for wt in candidates {
            match self.git.remove_worktree(&wt.path) {
                Ok(_) => result.removed.push(RemovedWorkspace {
                    path: wt.path.clone(),
                    kind: WorkspaceKind::Worktree,
                    branch: wt.branch.clone(),
                }),
                Err(e) => result.failed.push(FailedWorkspace {
                    path: wt.path.clone(),
                    error: e.to_string(),
                }),
            }
        }

        Ok(result)
    }

    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, crate::infra::git_error::GitError> {
        self.git.list_worktrees()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::GitWorkspaceInfo;
    use crate::infra::git_error::GitError;
    use std::path::{Path, PathBuf};

    // Mock GitOperations
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
                        // Generate mock output with the specified number of files
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
    fn test_create_workspace() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/tmp/repo"),
            project_name: "test-project".to_string(),
            worktrees: vec![],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let result = workspace.create(None);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("test-project"));
    }

    #[test]
    fn test_get_status_for_specific_worktree() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree1_info = GitWorkspaceInfo {
            path: worktree1_path.clone(),
            branch: Some("feature".to_string()),
        };
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                // Main worktree is always first
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                worktree1_info.clone(),
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![
                (repo_root.clone(), true, 5),
                (worktree1_path.clone(), true, 3),
            ],
            divergence: vec![(repo_root.clone(), 3, 0), (worktree1_path.clone(), 2, 0)],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let status = workspace.get_git_status(worktree1_info).unwrap();

        assert_eq!(status.path, worktree1_path);
        assert_eq!(status.branch, Some("feature".to_string()));
        assert!(status.has_uncommitted_changes);
        assert_eq!(status.uncommitted_file_count, 3);
        assert_eq!(status.commits_ahead, 2);
        assert_eq!(status.commits_behind, 0);
    }

    #[test]
    fn test_get_status_with_uncommitted_changes() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree1_info = GitWorkspaceInfo {
            path: worktree1_path.clone(),
            branch: Some("feature1".to_string()),
        };
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                worktree1_info.clone(),
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![(worktree1_path.clone(), true, 3)],
            divergence: vec![(worktree1_path.clone(), 2, 0)],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let status = workspace.get_git_status(worktree1_info).unwrap();

        assert_eq!(status.path, worktree1_path);
        assert!(status.has_uncommitted_changes);
        assert_eq!(status.uncommitted_file_count, 3);
    }

    #[test]
    fn test_get_status_for_clean_worktree() {
        let repo_root = PathBuf::from("/repo");
        let worktree2_path = PathBuf::from("/repo/worktree2");
        let worktree2_info = GitWorkspaceInfo {
            path: worktree2_path.clone(),
            branch: Some("feature2".to_string()),
        };
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                worktree2_info.clone(),
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![(worktree2_path.clone(), false, 0)],
            divergence: vec![(worktree2_path.clone(), 0, 0)],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let status = workspace.get_git_status(worktree2_info).unwrap();

        assert_eq!(status.path, worktree2_path);
        assert!(!status.has_uncommitted_changes);
        assert_eq!(status.uncommitted_file_count, 0);
        assert_eq!(status.commits_ahead, 0);
        assert_eq!(status.commits_behind, 0);
    }

    #[test]
    fn test_get_status_with_divergence() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree1_info = GitWorkspaceInfo {
            path: worktree1_path.clone(),
            branch: Some("feature1".to_string()),
        };
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                worktree1_info.clone(),
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![(worktree1_path.clone(), false, 0)],
            divergence: vec![(worktree1_path.clone(), 5, 1)],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let status = workspace.get_git_status(worktree1_info).unwrap();

        assert_eq!(status.commits_ahead, 5);
        assert_eq!(status.commits_behind, 1);
    }

    #[test]
    fn test_get_status_for_nonexistent_worktree() {
        let repo_root = PathBuf::from("/repo");
        let nonexistent_path = PathBuf::from("/repo/nonexistent");
        let nonexistent_info = GitWorkspaceInfo {
            path: nonexistent_path.clone(),
            branch: Some("feature".to_string()),
        };
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![GitWorkspaceInfo {
                path: repo_root.clone(),
                branch: Some("main".to_string()),
            }],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![],
            divergence: vec![],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let result = workspace.get_git_status(nonexistent_info);

        // Mock returns Ok with default values (no uncommitted changes, no divergence)
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.path, nonexistent_path);
        assert!(!status.has_uncommitted_changes);
        assert_eq!(status.commits_ahead, 0);
        assert_eq!(status.commits_behind, 0);
    }

    #[test]
    fn test_parse_uncommitted_changes_with_files() {
        let status_output = " M file1.txt\n M file2.txt\n M file3.txt\n";
        let (has_changes, count) =
            GitWorktreeWorkspace::<MockGit>::parse_uncommitted_changes(status_output);
        assert!(has_changes);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_parse_uncommitted_changes_empty() {
        let status_output = "";
        let (has_changes, count) =
            GitWorktreeWorkspace::<MockGit>::parse_uncommitted_changes(status_output);
        assert!(!has_changes);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_parse_uncommitted_changes_with_empty_lines() {
        let status_output = " M file1.txt\n\n M file2.txt\n";
        let (has_changes, count) =
            GitWorktreeWorkspace::<MockGit>::parse_uncommitted_changes(status_output);
        assert!(has_changes);
        assert_eq!(count, 2); // Should only count non-empty lines
    }

    #[test]
    fn test_get_worktree_statuses_excludes_main() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                GitWorkspaceInfo {
                    path: worktree1_path.clone(),
                    branch: Some("feature".to_string()),
                },
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![
                (repo_root.clone(), true, 5),
                (worktree1_path.clone(), true, 3),
            ],
            divergence: vec![(repo_root.clone(), 3, 0), (worktree1_path.clone(), 2, 0)],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let statuses = workspace.get_statuses(true).unwrap();

        // Should only return the non-main worktree
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].path, worktree1_path);
    }

    #[test]
    fn test_get_worktree_statuses_filters_clean() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree2_path = PathBuf::from("/repo/worktree2");
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                GitWorkspaceInfo {
                    path: worktree1_path.clone(),
                    branch: Some("feature1".to_string()),
                },
                GitWorkspaceInfo {
                    path: worktree2_path.clone(),
                    branch: Some("feature2".to_string()),
                },
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![
                (worktree1_path.clone(), true, 3),
                (worktree2_path.clone(), false, 0),
            ],
            divergence: vec![
                (worktree1_path.clone(), 2, 0),
                (worktree2_path.clone(), 0, 0),
            ],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let statuses = workspace.get_statuses(false).unwrap();

        // Should only return worktree1 which has changes
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].path, worktree1_path);
    }

    #[test]
    fn test_get_worktree_statuses_shows_all() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree2_path = PathBuf::from("/repo/worktree2");
        let mock_git = MockGit {
            repo_root: repo_root.clone(),
            project_name: "test-project".to_string(),
            worktrees: vec![
                GitWorkspaceInfo {
                    path: repo_root.clone(),
                    branch: Some("main".to_string()),
                },
                GitWorkspaceInfo {
                    path: worktree1_path.clone(),
                    branch: Some("feature1".to_string()),
                },
                GitWorkspaceInfo {
                    path: worktree2_path.clone(),
                    branch: Some("feature2".to_string()),
                },
            ],
            base_branch: "origin/main".to_string(),
            uncommitted_changes: vec![
                (worktree1_path.clone(), true, 3),
                (worktree2_path.clone(), false, 0),
            ],
            divergence: vec![
                (worktree1_path.clone(), 2, 0),
                (worktree2_path.clone(), 0, 0),
            ],
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None);
        let statuses = workspace.get_statuses(true).unwrap();

        // Should return both worktrees (excluding main)
        assert_eq!(statuses.len(), 2);
    }
}
