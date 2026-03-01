use std::collections::HashSet;
use std::path::PathBuf;

use uuid::Uuid;

use super::agent_launcher::LaunchError;
use super::agent_workspace::{
    CleanupError, CleanupResult, FailedWorkspace, GitStatus, RemovedWorkspace, SkippedWorkspace,
    StatusError, WorkspaceFactory, WorkspaceManager, commits_ahead_of_remote,
};
use super::silo_config::SiloConfig;
use super::workspace_kind::WorkspaceKind;
use crate::infra::git::{GitOperations, GitWorkspaceInfo};

pub struct GitWorktreeWorkspace<G: GitOperations> {
    git: G,
}

impl<G: GitOperations> GitWorktreeWorkspace<G> {
    pub fn new(git: G) -> Self {
        Self { git }
    }

    fn generate_worktree_path(&self) -> Result<PathBuf, LaunchError> {
        let base_dir = SiloConfig::get_silo_dir().ok_or_else(|| {
            LaunchError::AgentSpawnError("could not determine home directory".into())
        })?;

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

impl<G: GitOperations> WorkspaceFactory for GitWorktreeWorkspace<G> {
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        let worktree_path = self.generate_worktree_path()?;
        let worktree_name = worktree_path.file_name().unwrap().to_string_lossy();
        let branch_name = branch.unwrap_or_else(|| worktree_name.to_string());

        self.git.create_worktree(&worktree_path, &branch_name)?;

        Ok(worktree_path)
    }
}

impl<G: GitOperations> WorkspaceManager for GitWorktreeWorkspace<G> {
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
        force: bool,
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

        let base_branch = self.git.get_default_remote_branch().ok();

        for wt in candidates {
            if !force
                && let Some(ahead) = commits_ahead_of_remote(&self.git, &wt.path, &base_branch)
            {
                result.skipped.push(SkippedWorkspace {
                    path: wt.path.clone(),
                    kind: WorkspaceKind::Worktree,
                    branch: wt.branch.clone(),
                    commits_ahead: ahead,
                });
                continue;
            }

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
    use crate::infra::git::{GitWorkspaceInfo, MockGitOperations};

    // MockGit is deprecated in favour of MockGitOperations generated by mockall automock.
    #[deprecated(note = "Use MockGitOperations from mockall automock instead")]
    #[allow(dead_code)]
    struct MockGit {
        repo_root: PathBuf,
        project_name: String,
        worktrees: Vec<GitWorkspaceInfo>,
        base_branch: String,
        uncommitted_changes: Vec<(PathBuf, bool, usize)>,
        divergence: Vec<(PathBuf, usize, usize)>,
    }

    #[allow(deprecated)]
    impl GitOperations for MockGit {
        fn get_repo_root(&self) -> Result<PathBuf, crate::infra::git_error::GitError> {
            Ok(self.repo_root.clone())
        }

        fn get_project_name(&self) -> Result<String, crate::infra::git_error::GitError> {
            Ok(self.project_name.clone())
        }

        fn create_worktree(
            &self,
            _path: &std::path::Path,
            _branch: &str,
        ) -> Result<(), crate::infra::git_error::GitError> {
            Ok(())
        }

        fn list_worktrees(
            &self,
        ) -> Result<Vec<GitWorkspaceInfo>, crate::infra::git_error::GitError> {
            Ok(self.worktrees.clone())
        }

        fn remove_worktree(
            &self,
            _path: &std::path::Path,
        ) -> Result<(), crate::infra::git_error::GitError> {
            Ok(())
        }

        fn get_default_remote_branch(
            &self,
        ) -> Result<String, crate::infra::git_error::GitError> {
            Ok(self.base_branch.clone())
        }

        fn get_status_porcelain(
            &self,
            worktree_path: &std::path::Path,
        ) -> Result<String, crate::infra::git_error::GitError> {
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
            worktree_path: &std::path::Path,
            _base_branch: &str,
        ) -> Result<usize, crate::infra::git_error::GitError> {
            for (path, ahead, _) in &self.divergence {
                if path == worktree_path {
                    return Ok(*ahead);
                }
            }
            Ok(0)
        }

        fn count_commits_behind(
            &self,
            worktree_path: &std::path::Path,
            _base_branch: &str,
        ) -> Result<usize, crate::infra::git_error::GitError> {
            for (path, _, behind) in &self.divergence {
                if path == worktree_path {
                    return Ok(*behind);
                }
            }
            Ok(0)
        }

        fn clone_local(
            &self,
            _source: &std::path::Path,
            _dest: &std::path::Path,
        ) -> Result<(), crate::infra::git_error::GitError> {
            Ok(())
        }

        fn checkout_new_branch(
            &self,
            _path: &std::path::Path,
            _branch: &str,
        ) -> Result<(), crate::infra::git_error::GitError> {
            Ok(())
        }

        fn get_current_branch(
            &self,
            _path: &std::path::Path,
        ) -> Result<Option<String>, crate::infra::git_error::GitError> {
            Ok(None)
        }
    }

    #[test]
    fn test_create_workspace() {
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        mock_git
            .expect_create_worktree()
            .returning(|_, _| Ok(()));

        let workspace = GitWorktreeWorkspace::new(mock_git);
        let result = workspace.create(None);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("test-project"));
    }

    #[test]
    fn test_get_status_for_specific_worktree() {
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree1_info = GitWorkspaceInfo {
            path: worktree1_path.clone(),
            branch: Some("feature".to_string()),
        };

        let wt1 = worktree1_path.clone();
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git.expect_get_status_porcelain().returning(move |path| {
            if path == wt1 {
                Ok(" M file0.txt\n M file1.txt\n M file2.txt".to_string())
            } else {
                Ok(String::new())
            }
        });
        let wt1 = worktree1_path.clone();
        mock_git
            .expect_count_commits_ahead()
            .returning(move |path, _| if path == wt1 { Ok(2) } else { Ok(0) });
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
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
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree1_info = GitWorkspaceInfo {
            path: worktree1_path.clone(),
            branch: Some("feature1".to_string()),
        };

        let wt1 = worktree1_path.clone();
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git.expect_get_status_porcelain().returning(move |path| {
            if path == wt1 {
                Ok(" M file0.txt\n M file1.txt\n M file2.txt".to_string())
            } else {
                Ok(String::new())
            }
        });
        mock_git
            .expect_count_commits_ahead()
            .returning(|_, _| Ok(2));
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
        let status = workspace.get_git_status(worktree1_info).unwrap();

        assert_eq!(status.path, worktree1_path);
        assert!(status.has_uncommitted_changes);
        assert_eq!(status.uncommitted_file_count, 3);
    }

    #[test]
    fn test_get_status_for_clean_worktree() {
        let mut mock_git = MockGitOperations::new();
        let worktree2_path = PathBuf::from("/repo/worktree2");
        let worktree2_info = GitWorkspaceInfo {
            path: worktree2_path.clone(),
            branch: Some("feature2".to_string()),
        };

        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git
            .expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock_git
            .expect_count_commits_ahead()
            .returning(|_, _| Ok(0));
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
        let status = workspace.get_git_status(worktree2_info).unwrap();

        assert_eq!(status.path, worktree2_path);
        assert!(!status.has_uncommitted_changes);
        assert_eq!(status.uncommitted_file_count, 0);
        assert_eq!(status.commits_ahead, 0);
        assert_eq!(status.commits_behind, 0);
    }

    #[test]
    fn test_get_status_with_divergence() {
        let worktree1_path = PathBuf::from("/repo/worktree1");
        let worktree1_info = GitWorkspaceInfo {
            path: worktree1_path.clone(),
            branch: Some("feature1".to_string()),
        };

        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git
            .expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock_git
            .expect_count_commits_ahead()
            .returning(|_, _| Ok(5));
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(1));

        let workspace = GitWorktreeWorkspace::new(mock_git);
        let status = workspace.get_git_status(worktree1_info).unwrap();

        assert_eq!(status.commits_ahead, 5);
        assert_eq!(status.commits_behind, 1);
    }

    #[test]
    fn test_get_status_for_nonexistent_worktree() {
        let nonexistent_path = PathBuf::from("/repo/nonexistent");
        let nonexistent_info = GitWorkspaceInfo {
            path: nonexistent_path.clone(),
            branch: Some("feature".to_string()),
        };

        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git
            .expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock_git
            .expect_count_commits_ahead()
            .returning(|_, _| Ok(0));
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
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
            GitWorktreeWorkspace::<MockGitOperations>::parse_uncommitted_changes(status_output);
        assert!(has_changes);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_parse_uncommitted_changes_empty() {
        let status_output = "";
        let (has_changes, count) =
            GitWorktreeWorkspace::<MockGitOperations>::parse_uncommitted_changes(status_output);
        assert!(!has_changes);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_parse_uncommitted_changes_with_empty_lines() {
        let status_output = " M file1.txt\n\n M file2.txt\n";
        let (has_changes, count) =
            GitWorktreeWorkspace::<MockGitOperations>::parse_uncommitted_changes(status_output);
        assert!(has_changes);
        assert_eq!(count, 2); // Should only count non-empty lines
    }

    #[test]
    fn test_get_worktree_statuses_excludes_main() {
        let repo_root = PathBuf::from("/repo");
        let worktree1_path = PathBuf::from("/repo/worktree1");

        let worktrees = vec![
            GitWorkspaceInfo {
                path: repo_root.clone(),
                branch: Some("main".to_string()),
            },
            GitWorkspaceInfo {
                path: worktree1_path.clone(),
                branch: Some("feature".to_string()),
            },
        ];

        let wt1 = worktree1_path.clone();
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_list_worktrees()
            .return_once(move || Ok(worktrees));
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git.expect_get_status_porcelain().returning(move |path| {
            if path == wt1 {
                Ok(" M file0.txt\n M file1.txt\n M file2.txt".to_string())
            } else {
                Ok(String::new())
            }
        });
        let wt1 = worktree1_path.clone();
        mock_git
            .expect_count_commits_ahead()
            .returning(move |path, _| if path == wt1 { Ok(2) } else { Ok(0) });
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
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

        let worktrees = vec![
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
        ];

        let wt1 = worktree1_path.clone();
        let wt1b = worktree1_path.clone();
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_list_worktrees()
            .return_once(move || Ok(worktrees));
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git.expect_get_status_porcelain().returning(move |path| {
            if path == wt1 {
                Ok(" M file0.txt\n M file1.txt\n M file2.txt".to_string())
            } else {
                Ok(String::new())
            }
        });
        mock_git
            .expect_count_commits_ahead()
            .returning(move |path, _| if path == wt1b { Ok(2) } else { Ok(0) });
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
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

        let worktrees = vec![
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
        ];

        let wt1 = worktree1_path.clone();
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_list_worktrees()
            .return_once(move || Ok(worktrees));
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git.expect_get_status_porcelain().returning(move |path| {
            if path == wt1 {
                Ok(" M file0.txt\n M file1.txt\n M file2.txt".to_string())
            } else {
                Ok(String::new())
            }
        });
        mock_git
            .expect_count_commits_ahead()
            .returning(|_, _| Ok(0));
        mock_git
            .expect_count_commits_behind()
            .returning(|_, _| Ok(0));

        let workspace = GitWorktreeWorkspace::new(mock_git);
        let statuses = workspace.get_statuses(true).unwrap();

        // Should return both worktrees (excluding main)
        assert_eq!(statuses.len(), 2);
    }

    #[test]
    fn test_cleanup_skips_worktrees_with_unpushed_commits() {
        let repo_root = PathBuf::from("/repo");
        let silo_dir = PathBuf::from("/Users/test/.silo");
        let worktree1_path = silo_dir.join("worktree1");
        let worktree2_path = silo_dir.join("worktree2");

        let worktrees = vec![
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
        ];

        let wt1 = worktree1_path.clone();
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_list_worktrees()
            .return_once(move || Ok(worktrees));
        mock_git
            .expect_get_repo_root()
            .returning(move || Ok(repo_root.clone()));
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        mock_git
            .expect_count_commits_ahead()
            .returning(move |path, _| if path == wt1 { Ok(3) } else { Ok(0) });
        mock_git.expect_remove_worktree().returning(|_| Ok(()));

        let workspace = GitWorktreeWorkspace::new(mock_git);
        let active = HashSet::new();
        let result = workspace.cleanup(&active, true, false).unwrap(); // Use all: true to include all worktrees

        // worktree1 should be skipped, worktree2 should be removed
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0].path, worktree2_path);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].path, worktree1_path);
        assert_eq!(result.skipped[0].commits_ahead, 3);
    }
}
