use std::path::PathBuf;

use uuid::Uuid;

use super::agent_launcher::LaunchError;
use super::agent_workspace::AgentWorkspace;
use super::silo_config::SiloConfig;
use crate::infra::git::GitOperations;

pub struct GitWorktreeWorkspace<G: GitOperations> {
    git: G,
    worktree_base: Option<PathBuf>,
    branch: Option<String>,
}

impl<G: GitOperations> GitWorktreeWorkspace<G> {
    pub fn new(git: G, worktree_base: Option<PathBuf>, branch: Option<String>) -> Self {
        Self {
            git,
            worktree_base,
            branch,
        }
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
}

impl<G: GitOperations> AgentWorkspace for GitWorktreeWorkspace<G> {
    fn create(&self) -> Result<PathBuf, LaunchError> {
        let worktree_path = self.generate_worktree_path()?;
        let worktree_name = worktree_path.file_name().unwrap().to_string_lossy();
        let branch_name = self
            .branch
            .clone()
            .unwrap_or_else(|| worktree_name.to_string());

        println!("Creating worktree at: {}", worktree_path.display());
        println!("Branch: {}", branch_name);

        self.git.create_worktree(&worktree_path, &branch_name)?;

        Ok(worktree_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::WorktreeInfo;
    use crate::infra::git_error::GitError;
    use std::path::{Path, PathBuf};

    // Mock GitOperations
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

        fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, GitError> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_create_workspace() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/tmp/repo"),
            project_name: "test-project".to_string(),
        };

        let workspace = GitWorktreeWorkspace::new(mock_git, None, None);
        let result = workspace.create();

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("test-project"));
    }
}
