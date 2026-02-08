use crate::infra::git_error::GitError;
use crate::infra::terminal::Terminal;
use crate::services::agent_workspace::AgentWorkspace;
use std::fmt;
use std::os::unix::process::CommandExt;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchMode {
    ExecReplace,
    NewTab,
    SplitPane,
}

#[derive(Debug)]
pub enum LaunchError {
    AgentSpawnError(String),
    Git(GitError),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::AgentSpawnError(msg) => write!(f, "failed to spawn agent: {}", msg),
            LaunchError::Git(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LaunchError::Git(err) => Some(err),
            LaunchError::AgentSpawnError(_) => None,
        }
    }
}

impl From<GitError> for LaunchError {
    fn from(err: GitError) -> Self {
        LaunchError::Git(err)
    }
}

pub struct AgentLauncher {
    workspace: Box<dyn AgentWorkspace>,
    terminal: Option<Box<dyn Terminal>>,
    launch_mode: LaunchMode,
}

impl AgentLauncher {
    pub fn new(
        workspace: Box<dyn AgentWorkspace>,
        terminal: Option<Box<dyn Terminal>>,
        launch_mode: LaunchMode,
    ) -> Self {
        Self {
            workspace,
            terminal,
            launch_mode,
        }
    }

    pub fn launch(&self) -> Result<(), LaunchError> {
        // Step 1: Create workspace
        let workspace_path = self.workspace.create()?;

        // Step 2: Launch agent in the workspace
        match self.launch_mode {
            LaunchMode::ExecReplace => {
                println!("Launching claude in worktree...");
                let err = Command::new("claude").current_dir(&workspace_path).exec();
                Err(LaunchError::AgentSpawnError(err.to_string()))
            }
            LaunchMode::NewTab => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for new tab".to_string())
                })?;
                println!("Opening new tab for claude in worktree...");
                match terminal.open_tab(&workspace_path) {
                    Ok(()) => {
                        println!("Agent launched in new tab at: {}", workspace_path.display());
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Failed to open new tab: {}", e);
                        eprintln!("Falling back to launching claude in current process...");
                        let err = Command::new("claude").current_dir(&workspace_path).exec();
                        Err(LaunchError::AgentSpawnError(err.to_string()))
                    }
                }
            }
            LaunchMode::SplitPane => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for split pane".to_string())
                })?;
                println!("Opening split pane for claude in worktree...");
                match terminal.split_pane(&workspace_path) {
                    Ok(()) => {
                        println!(
                            "Agent launched in split pane at: {}",
                            workspace_path.display()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Failed to open split pane: {}", e);
                        eprintln!("Falling back to launching claude in current process...");
                        let err = Command::new("claude").current_dir(&workspace_path).exec();
                        Err(LaunchError::AgentSpawnError(err.to_string()))
                    }
                }
            }
        }
    }
}
