use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use uuid::Uuid;

use super::agent_launcher::{AgentLauncher, LaunchError, LaunchMode};
use crate::infra::git::GitOperations;
use crate::infra::terminal::Terminal;

pub struct GitWorktreeLauncher<G: GitOperations> {
    git: G,
    worktree_base: Option<PathBuf>,
    branch: Option<String>,
    terminal: Option<Box<dyn Terminal>>,
}

impl<G: GitOperations> GitWorktreeLauncher<G> {
    pub fn new(
        git: G,
        worktree_base: Option<PathBuf>,
        branch: Option<String>,
        terminal: Option<Box<dyn Terminal>>,
    ) -> Self {
        Self {
            git,
            worktree_base,
            branch,
            terminal,
        }
    }

    fn generate_worktree_path(&self) -> Result<PathBuf, LaunchError> {
        let repo_root = self.git.get_repo_root()?;
        let base_dir = self.worktree_base.clone().unwrap_or_else(|| {
            repo_root
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| repo_root.to_path_buf())
        });

        let worktree_name = format!(
            "{}-{}",
            self.git.get_project_name()?,
            &Uuid::new_v4().to_string()[..8]
        );
        Ok(base_dir.join(&worktree_name))
    }
}

impl<G: GitOperations> AgentLauncher for GitWorktreeLauncher<G> {
    fn launch(&self, launch_mode: LaunchMode) -> Result<(), LaunchError> {
        let worktree_path = self.generate_worktree_path()?;
        let worktree_name = worktree_path.file_name().unwrap().to_string_lossy();
        let branch_name = self
            .branch
            .clone()
            .unwrap_or_else(|| worktree_name.to_string());

        println!("Creating worktree at: {}", worktree_path.display());
        println!("Branch: {}", branch_name);

        self.git.create_worktree(&worktree_path, &branch_name)?;

        match launch_mode {
            LaunchMode::ExecReplace => {
                println!("Launching claude in worktree...");
                let err = Command::new("claude").current_dir(&worktree_path).exec();
                Err(LaunchError::AgentSpawnError(err.to_string()))
            }
            LaunchMode::NewTab => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for new tab".to_string())
                })?;
                println!("Opening new tab for claude in worktree...");
                match terminal.open_tab(&worktree_path) {
                    Ok(()) => {
                        println!("Agent launched in new tab at: {}", worktree_path.display());
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Failed to open new tab: {}", e);
                        eprintln!("Falling back to launching claude in current process...");
                        let err = Command::new("claude").current_dir(&worktree_path).exec();
                        Err(LaunchError::AgentSpawnError(err.to_string()))
                    }
                }
            }
            LaunchMode::SplitPane => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for split pane".to_string())
                })?;
                println!("Opening split pane for claude in worktree...");
                match terminal.split_pane(&worktree_path) {
                    Ok(()) => {
                        println!(
                            "Agent launched in split pane at: {}",
                            worktree_path.display()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Failed to open split pane: {}", e);
                        eprintln!("Falling back to launching claude in current process...");
                        let err = Command::new("claude").current_dir(&worktree_path).exec();
                        Err(LaunchError::AgentSpawnError(err.to_string()))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }

    #[test]
    fn test_launch_creates_worktree_and_launches_agent() {
        let mock_git = MockGit {
            repo_root: PathBuf::from("/tmp/repo"),
            project_name: "test-project".to_string(),
        };

        let launcher = GitWorktreeLauncher::new(mock_git, None, None, None);

        // We expect an error because Command::exec will fail in test environment
        // but we can check if the worktree creation was attempted.
        let result = launcher.launch(LaunchMode::ExecReplace);
        assert!(result.is_err());

        if let Err(LaunchError::AgentSpawnError(_)) = result {
            // This confirms that the launch process proceeded past worktree creation
        } else {
            panic!("Expected AgentSpawnError");
        }
    }
}
