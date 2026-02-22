use std::collections::HashSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::agent_launcher::LaunchError;
use super::agent_workspace::{
    AgentWorkspaceManager, CleanupError, CleanupResult, FailedWorkspace, GitStatus,
    RemovedWorkspace, StatusError, WorkspaceKind,
};
use super::silo_config::SiloConfig;
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

impl<G: GitOperations> AgentWorkspaceManager for GitCheckoutWorkspace<G> {
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        let repo_root = self.git.get_repo_root()?;
        let dest = self.generate_checkout_path()?;
        let dest_name = dest.file_name().unwrap().to_string_lossy();
        let branch_name = branch.unwrap_or_else(|| dest_name.to_string());

        self.git.clone_local(&repo_root, &dest)?;
        self.git.checkout_new_branch(&dest, &branch_name)?;

        Ok(dest)
    }

    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        let base_dir = SiloConfig::get_silo_dir().unwrap();
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
    ) -> Result<CleanupResult, CleanupError> {
        let silo_dir = SiloConfig::get_silo_dir();
        let project_prefix = format!("{}-", self.git.get_project_name()?);

        let candidates = if let Some(ref silo) = silo_dir {
            find_checkout_dirs(silo, &project_prefix, exclude_paths)
        } else {
            vec![]
        };

        let mut result = CleanupResult::default();

        for path in candidates {
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
    use crate::infra::git::GitWorkspaceInfo;
    use crate::infra::git_error::GitError;
    use std::path::{Path, PathBuf};

    struct MockGit {
        repo_root: PathBuf,
        project_name: String,
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
            Ok(vec![])
        }

        fn remove_worktree(&self, _path: &Path) -> Result<(), GitError> {
            Ok(())
        }

        fn get_default_remote_branch(&self) -> Result<String, GitError> {
            Ok("origin/main".to_string())
        }

        fn get_status_porcelain(&self, _worktree_path: &Path) -> Result<String, GitError> {
            Ok(String::new())
        }

        fn count_commits_ahead(
            &self,
            _worktree_path: &Path,
            _base_branch: &str,
        ) -> Result<usize, GitError> {
            Ok(0)
        }

        fn count_commits_behind(
            &self,
            _worktree_path: &Path,
            _base_branch: &str,
        ) -> Result<usize, GitError> {
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
    fn test_create_checkout_workspace() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/tmp/repo"),
            project_name: "test-project".to_string(),
        };

        let workspace = GitCheckoutWorkspace::new(mock_git);
        let result = workspace.create(None);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("test-project"));
    }

    #[test]
    fn test_create_checkout_workspace_custom_branch() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/tmp/repo"),
            project_name: "my-project".to_string(),
        };

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
}
