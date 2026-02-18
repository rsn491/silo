use crate::infra::agent::Agent;
use crate::infra::git_error::GitError;
use crate::infra::terminal::Terminal;
use crate::services::agent_workspace::AgentWorkspaceManager;
use std::fmt;

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

pub struct AgentLauncher<W, T>
where
    W: AgentWorkspaceManager,
    T: Terminal,
{
    workspace: W,
    terminal: Option<T>,
    launch_mode: LaunchMode,
    agent: Agent,
    branch: Option<String>,
}

impl<W, T> AgentLauncher<W, T>
where
    W: AgentWorkspaceManager,
    T: Terminal,
{
    pub fn new(
        workspace: W,
        terminal: Option<T>,
        launch_mode: LaunchMode,
        agent: Agent,
        branch: Option<String>,
    ) -> Self {
        Self {
            workspace,
            terminal,
            launch_mode,
            agent,
            branch,
        }
    }

    fn launch_in_workspace(&self, workspace_path: &std::path::Path) -> Result<(), LaunchError> {
        let status = self
            .agent
            .command()
            .current_dir(workspace_path)
            .status()
            .map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;
        if status.success() {
            println!(
                "\nAgent exited. To resume, run: silo launch from {}",
                workspace_path.display()
            );
        }
        Ok(())
    }

    pub fn launch(&self) -> Result<(), LaunchError> {
        // Step 1: Create workspace
        let workspace_path = self.workspace.create(self.branch.clone())?;

        // Step 2: Launch agent in the workspace
        match self.launch_mode {
            LaunchMode::ExecReplace => {
                println!("Launching {:?} in worktree...", self.agent);
                self.launch_in_workspace(&workspace_path)
            }
            LaunchMode::NewTab => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for new tab".to_string())
                })?;
                println!("Opening new tab for {:?} in worktree...", self.agent);
                match terminal.open_tab(&workspace_path, &self.agent) {
                    Ok(()) => {
                        println!("Agent launched in new tab at: {}", workspace_path.display());
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Failed to open new tab: {}", e);
                        eprintln!(
                            "Falling back to launching {:?} in current process...",
                            self.agent
                        );
                        self.launch_in_workspace(&workspace_path)
                    }
                }
            }
            LaunchMode::SplitPane => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for split pane".to_string())
                })?;
                println!("Opening split pane for {:?} in worktree...", self.agent);
                match terminal.split_pane(&workspace_path, &self.agent) {
                    Ok(()) => {
                        println!(
                            "Agent launched in split pane at: {}",
                            workspace_path.display()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Failed to open split pane: {}", e);
                        eprintln!(
                            "Falling back to launching {:?} in current process...",
                            self.agent
                        );
                        self.launch_in_workspace(&workspace_path)
                    }
                }
            }
        }
    }
}
