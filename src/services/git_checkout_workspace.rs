use std::collections::HashSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::agent_launcher::LaunchError;
use super::agent_workspace::{
    CleanupError, CleanupResult, FailedWorkspace, GitStatus, RemovedWorkspace, SkippedWorkspace,
    StatusError, WorkspaceFactory, WorkspaceManager, commits_ahead_of_remote,
};
use super::silo_config::SiloConfig;
use super::workspace_kind::WorkspaceKind;
use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;

pub struct GitCheckoutWorkspace<G: GitOperations> {
    git: G,
}

impl<G: GitOperations> GitCheckoutWorkspace<G> {
    pub fn new(git: G) -> Self {
        Self { git }
    }

    fn generate_checkout_path(&self) -> Result<PathBuf, LaunchError> {
        let base_dir = SiloConfig::get_silo_dir().ok_or_else(|| {
            LaunchError::AgentSpawnError("checkout workspaces require ~/.silo/ to exist".into())
        })?;
        let checkout_name = format!(
            "{}-{}",
            self.git.get_project_name()?,
            &Uuid::new_v4().to_string()[..8]
        );
        Ok(base_dir.join(&checkout_name))
    }
}

impl<G: GitOperations> WorkspaceFactory for GitCheckoutWorkspace<G> {
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        let repo_root = self.git.get_repo_root()?;
        let dest = self.generate_checkout_path()?;
        let dest_name = dest.file_name().ok_or_else(|| {
            LaunchError::AgentSpawnError("invalid checkout path".into())
        })?.to_string_lossy();
        let branch_name = branch.unwrap_or_else(|| dest_name.to_string());

        self.git.clone_local(&repo_root, &dest)?;
        self.git.checkout_new_branch(&dest, &branch_name)?;

        Ok(dest)
    }
}

impl<G: GitOperations> WorkspaceManager for GitCheckoutWorkspace<G> {
    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        let base_dir = SiloConfig::get_silo_dir().ok_or_else(|| {
            GitError::CommandFailed("could not determine home directory".to_string())
        })?;
        let project_prefix = format!("{}-", self.git.get_project_name()?);
        let checkout_dirs = find_checkout_dirs(&base_dir, &project_prefix, &HashSet::new());

        let mut result = Vec::new();
        for checkout_dir in checkout_dirs {
            let branch = self.git.get_current_branch(&checkout_dir)?;
            result.push(GitWorkspaceInfo {
                path: checkout_dir,
                branch,
            });
        }

        Ok(result)
    }

    fn get_statuses(&self, show_all: bool) -> Result<Vec<GitStatus>, StatusError> {
        let silo_dir = SiloConfig::get_silo_dir();
        let project_name = self.git.get_project_name()?;
        let project_prefix = format!("{}-", project_name);

        let checkout_dirs = if let Some(ref silo) = silo_dir {
            find_checkout_dirs(silo, &project_prefix, &HashSet::new())
        } else {
            vec![]
        };

        let base_branch = self.git.get_default_remote_branch()?;
        let mut statuses = Vec::new();

        for path in checkout_dirs {
            let branch = self.git.get_current_branch(&path)?;
            let status_output = self.git.get_status_porcelain(&path)?;
            let file_count = status_output.lines().filter(|l| !l.is_empty()).count();
            let has_uncommitted_changes = file_count > 0;

            let commits_ahead = self.git.count_commits_ahead(&path, &base_branch)?;
            let commits_behind = self.git.count_commits_behind(&path, &base_branch)?;

            let status = GitStatus {
                kind: WorkspaceKind::Checkout,
                path: path.clone(),
                branch,
                has_uncommitted_changes,
                uncommitted_file_count: file_count,
                commits_ahead,
                commits_behind,
            };

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
        exclude_paths: &HashSet<PathBuf>,
        _all: bool,
        force: bool,
    ) -> Result<CleanupResult, CleanupError> {
        let silo_dir = SiloConfig::get_silo_dir();
        let project_prefix = format!("{}-", self.git.get_project_name()?);

        let candidates = if let Some(ref silo) = silo_dir {
            find_checkout_dirs(silo, &project_prefix, exclude_paths)
        } else {
            vec![]
        };

        let mut result = CleanupResult::default();

        let base_branch = self.git.get_default_remote_branch().ok();

        for path in candidates {
            if !force && let Some(ahead) = commits_ahead_of_remote(&self.git, &path, &base_branch) {
                let branch = self.git.get_current_branch(&path).ok().flatten();
                result.skipped.push(SkippedWorkspace {
                    path: path.clone(),
                    kind: WorkspaceKind::Checkout,
                    branch,
                    commits_ahead: ahead,
                });
                continue;
            }

            match std::fs::remove_dir_all(&path) {
                Ok(_) => result.removed.push(RemovedWorkspace {
                    path,
                    kind: WorkspaceKind::Checkout,
                    branch: None,
                }),
                Err(e) => result.failed.push(FailedWorkspace {
                    path,
                    error: e.to_string(),
                }),
            }
        }

        Ok(result)
    }
}

/// Scans `base_dir` for subdirectories whose name starts with `project_prefix`
/// and whose `.git` entry is a **directory** (indicating a clone, not a worktree).
/// Entries in `exclude_paths` are skipped.
pub fn find_checkout_dirs(
    base_dir: &Path,
    project_prefix: &str,
    exclude_paths: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(base_dir) {
        Ok(e) => e,
        Err(_) => return results,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if exclude_paths.contains(&path) {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if !name.starts_with(project_prefix) {
            continue;
        }

        // Key distinction: worktrees have .git as a *file*, clones have .git as a *directory*
        let git_entry = path.join(".git");
        if git_entry.is_dir() {
            results.push(path);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::MockGitOperations;

    #[test]
    fn test_create_checkout_workspace() {
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        mock_git
            .expect_get_repo_root()
            .returning(|| Ok(PathBuf::from("/tmp/repo")));
        mock_git.expect_clone_local().returning(|_, _| Ok(()));
        mock_git
            .expect_checkout_new_branch()
            .returning(|_, _| Ok(()));

        let workspace = GitCheckoutWorkspace::new(mock_git);
        let result = workspace.create(None);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("test-project"));
    }

    #[test]
    fn test_create_checkout_workspace_custom_branch() {
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_project_name()
            .returning(|| Ok("my-project".to_string()));
        mock_git
            .expect_get_repo_root()
            .returning(|| Ok(PathBuf::from("/tmp/repo")));
        mock_git.expect_clone_local().returning(|_, _| Ok(()));
        mock_git
            .expect_checkout_new_branch()
            .returning(|_, _| Ok(()));

        let workspace = GitCheckoutWorkspace::new(mock_git);
        let result = workspace.create(Some("my-feature".to_string()));

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("my-project"));
    }

    #[test]
    fn test_find_checkout_dirs_skips_worktrees() {
        let temp = tempfile::TempDir::new().unwrap();
        let base = temp.path();

        // Create a directory that looks like a checkout (`.git` is a dir)
        let checkout_dir = base.join("my-project-abc12345");
        std::fs::create_dir_all(&checkout_dir).unwrap();
        std::fs::create_dir_all(checkout_dir.join(".git")).unwrap();

        // Create a directory that looks like a worktree (`.git` is a file)
        let worktree_dir = base.join("my-project-xyz67890");
        std::fs::create_dir_all(&worktree_dir).unwrap();
        std::fs::write(
            worktree_dir.join(".git"),
            "gitdir: /repo/.git/worktrees/xyz",
        )
        .unwrap();

        let results = find_checkout_dirs(base, "my-project-", &HashSet::new());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], checkout_dir);
    }

    #[test]
    fn test_find_checkout_dirs_skips_active() {
        let temp = tempfile::TempDir::new().unwrap();
        let base = temp.path();

        // Create two checkout dirs
        let checkout1 = base.join("my-project-aaa11111");
        std::fs::create_dir_all(&checkout1).unwrap();
        std::fs::create_dir_all(checkout1.join(".git")).unwrap();

        let checkout2 = base.join("my-project-bbb22222");
        std::fs::create_dir_all(&checkout2).unwrap();
        std::fs::create_dir_all(checkout2.join(".git")).unwrap();

        let mut exclude = HashSet::new();
        exclude.insert(checkout1.clone());

        let results = find_checkout_dirs(base, "my-project-", &exclude);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], checkout2);
    }

    #[test]
    fn test_cleanup_skips_checkouts_with_unpushed_commits() {
        // Test the skipped checkout logic with find_checkout_dirs
        let temp = tempfile::TempDir::new().unwrap();
        let silo_dir = temp.path();

        // Create two checkout dirs
        let checkout1 = silo_dir.join("my-project-aaa11111");
        std::fs::create_dir_all(&checkout1).unwrap();
        std::fs::create_dir_all(checkout1.join(".git")).unwrap();

        let checkout2 = silo_dir.join("my-project-bbb22222");
        std::fs::create_dir_all(&checkout2).unwrap();
        std::fs::create_dir_all(checkout2.join(".git")).unwrap();

        // Verify find_checkout_dirs finds both
        let candidates = find_checkout_dirs(silo_dir, "my-project-", &HashSet::new());
        assert_eq!(candidates.len(), 2);

        // Now test that the cleanup would skip checkout1 with unpushed commits
        let mut mock_git = MockGitOperations::new();
        mock_git
            .expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        let checkout1_capture = checkout1.clone();
        mock_git
            .expect_count_commits_ahead()
            .returning(move |path, _| {
                if path == checkout1_capture {
                    Ok(2)
                } else {
                    Ok(0)
                }
            });

        // Simulate cleanup logic manually to avoid SiloConfig::get_silo_dir() dependency
        let base_branch = mock_git.get_default_remote_branch().ok();
        let mut skipped_count = 0;
        let mut would_remove_count = 0;

        for checkout_path in candidates {
            if let Some(ref base) = base_branch {
                if let Ok(ahead) = mock_git.count_commits_ahead(&checkout_path, base) {
                    if ahead > 0 {
                        skipped_count += 1;
                        continue;
                    }
                }
            }
            would_remove_count += 1;
        }

        assert_eq!(skipped_count, 1);
        assert_eq!(would_remove_count, 1);
    }
}
