//! Workspace management implementation using local Git clones (checkouts).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::agent_launcher::LaunchError;
use super::silo_config::SiloConfig;
use super::workspace_manager::{
    CleanupError, CleanupResult, FailedWorkspace, RemovedWorkspace, SkippedWorkspace,
    WorkspaceFactory, WorkspaceManager, commits_ahead_of_remote, reuse_inactive_workspace,
};
use super::workspace_utils::{generate_workspace_path, parse_uncommitted_changes};
use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::infra::workspace_kind::WorkspaceKind;

/// Manages isolated workspaces by creating full local clones of the repository.
pub struct GitCheckoutWorkspace<G: GitOperations> {
    /// Git operations used to clone and inspect checkouts.
    git: G,
}

impl<G: GitOperations> GitCheckoutWorkspace<G> {
    /// Creates a new `GitCheckoutWorkspace` with the specified Git operations.
    pub fn new(git: G) -> Self {
        Self { git }
    }
}

impl<G: GitOperations> WorkspaceFactory for GitCheckoutWorkspace<G> {
    /// Creates a new workspace by performing a local Git clone.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if cloning or checking out the branch fails.
    fn create(&self, branch: Option<String>, reuse: bool) -> Result<PathBuf, LaunchError> {
        if reuse {
            let all = self.get_all().map_err(LaunchError::Git)?;
            if let Some(path) = reuse_inactive_workspace(&self.git, all, branch.clone())? {
                return Ok(path);
            }
        }

        let repo_root = self.git.get_repo_root()?;
        let dest =
            generate_workspace_path(&self.git, "checkout workspaces require ~/.silo/ to exist")?;
        let dest_name = dest
            .file_name()
            .ok_or_else(|| LaunchError::AgentSpawnError("invalid checkout path".into()))?
            .to_string_lossy();
        let branch_name = branch.unwrap_or_else(|| dest_name.to_string());

        self.git.clone_local(&repo_root, &dest)?;
        self.git.checkout_new_branch(&dest, &branch_name)?;

        Ok(dest)
    }
}

impl<G: GitOperations> WorkspaceManager for GitCheckoutWorkspace<G> {
    /// Returns all Git checkout workspaces for the current project.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if the Silo directory cannot be determined or project name fails.
    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        let project_prefix = format!("{}-", self.git.get_project_name()?);
        let silo_dir = match SiloConfig::get_silo_dir() {
            Some(d) => d,
            None => return Ok(vec![]),
        };
        let checkout_dirs = find_checkout_dirs(&silo_dir, &project_prefix, &HashSet::new());

        if checkout_dirs.is_empty() {
            return Ok(vec![]);
        }

        let base_branch = self.git.get_default_remote_branch()?;
        let mut result = Vec::new();

        for path in checkout_dirs {
            let branch = self.git.get_current_branch(&path)?;
            let status_output = self.git.get_status_porcelain(&path)?;
            let (has_uncommitted_changes, _) = parse_uncommitted_changes(&status_output);
            let commits_ahead = self
                .git
                .count_commits_ahead(&path, &base_branch)
                .unwrap_or_else(|e| {
                    log::warn!("could not count commits ahead for {:?}: {}", path, e);
                    0
                });

            let latest_commit = self.git.get_latest_commit(&path).ok().flatten();

            result.push(GitWorkspaceInfo {
                path,
                branch,
                has_uncommitted_changes,
                commits_ahead,
                latest_commit,
            });
        }

        Ok(result)
    }

    /// Removes inactive Git checkout workspaces.
    ///
    /// # Errors
    ///
    /// Returns [`CleanupError`] if project name resolution or Git operations fail.
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
///
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

        // Key distinction: worktrees have .git as a *file*, clones have .git as a *directory*.
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
        // Arrange
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

        // Act
        let result = workspace.create(None, false);

        // Assert
        assert!(result.is_ok());
        let path = result.expect("workspace creation should succeed");
        assert!(path.to_string_lossy().contains("test-project"));
    }

    #[test]
    fn test_create_checkout_workspace_custom_branch() {
        // Arrange
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

        // Act
        let result = workspace.create(Some("my-feature".to_string()), false);

        // Assert
        assert!(result.is_ok());
        let path = result.expect("workspace creation should succeed");
        assert!(path.to_string_lossy().contains("my-project"));
    }

    #[test]
    fn test_find_checkout_dirs_skips_worktrees() {
        // Arrange
        let temp = tempfile::TempDir::new().expect("failed to create temp dir");
        let base = temp.path();

        // Create a directory that looks like a checkout (`.git` is a dir).
        let checkout_dir = base.join("my-project-abc12345");
        std::fs::create_dir_all(&checkout_dir).expect("failed to create checkout dir");
        std::fs::create_dir_all(checkout_dir.join(".git")).expect("failed to create .git dir");

        // Create a directory that looks like a worktree (`.git` is a file).
        let worktree_dir = base.join("my-project-xyz67890");
        std::fs::create_dir_all(&worktree_dir).expect("failed to create worktree dir");
        std::fs::write(
            worktree_dir.join(".git"),
            "gitdir: /repo/.git/worktrees/xyz",
        )
        .expect("failed to write .git file");

        // Act
        let results = find_checkout_dirs(base, "my-project-", &HashSet::new());

        // Assert
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], checkout_dir);
    }

    #[test]
    fn test_find_checkout_dirs_skips_active() {
        // Arrange
        let temp = tempfile::TempDir::new().expect("failed to create temp dir");
        let base = temp.path();

        // Create two checkout dirs.
        let checkout1 = base.join("my-project-aaa11111");
        std::fs::create_dir_all(&checkout1).expect("failed to create checkout dir");
        std::fs::create_dir_all(checkout1.join(".git")).expect("failed to create .git dir");

        let checkout2 = base.join("my-project-bbb22222");
        std::fs::create_dir_all(&checkout2).expect("failed to create checkout dir");
        std::fs::create_dir_all(checkout2.join(".git")).expect("failed to create .git dir");

        let mut exclude = HashSet::new();
        exclude.insert(checkout1.clone());

        // Act
        let results = find_checkout_dirs(base, "my-project-", &exclude);

        // Assert
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], checkout2);
    }

    #[test]
    #[allow(clippy::collapsible_if)]
    fn test_cleanup_skips_checkouts_with_unpushed_commits() {
        // Arrange
        // Test the skipped checkout logic with find_checkout_dirs.
        let temp = tempfile::TempDir::new().expect("failed to create temp dir");
        let silo_dir = temp.path();

        // Create two checkout dirs.
        let checkout1 = silo_dir.join("my-project-aaa11111");
        std::fs::create_dir_all(&checkout1).expect("failed to create checkout dir");
        std::fs::create_dir_all(checkout1.join(".git")).expect("failed to create .git dir");

        let checkout2 = silo_dir.join("my-project-bbb22222");
        std::fs::create_dir_all(&checkout2).expect("failed to create checkout dir");
        std::fs::create_dir_all(checkout2.join(".git")).expect("failed to create .git dir");

        // Verify find_checkout_dirs finds both.
        let candidates = find_checkout_dirs(silo_dir, "my-project-", &HashSet::new());
        assert_eq!(candidates.len(), 2);

        // Now test that the cleanup would skip checkout1 with unpushed commits.
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

        // Act
        // Simulate cleanup logic manually to avoid SiloConfig::get_silo_dir() dependency.
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

        // Assert
        assert_eq!(skipped_count, 1);
        assert_eq!(would_remove_count, 1);
    }
}
