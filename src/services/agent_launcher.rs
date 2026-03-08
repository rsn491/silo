//! Logic for launching AI agents in isolated workspaces.

use crate::infra::agent::Agent;
use crate::infra::git_error::GitError;
use crate::infra::terminal::{Terminal, TerminalError};
use crate::services::agent_workspace::WorkspaceFactory;
use crate::services::workspace_lock::WorkspaceLock;
use thiserror::Error;

/// Modes for launching an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchMode {
    /// Replace the current process with the agent.
    ExecReplace,
    /// Launch the agent in a new terminal tab.
    NewTab,
    /// Launch the agent in a new terminal split pane.
    SplitPane,
}

/// Errors that can occur when launching an agent.
#[derive(Debug, Error)]
pub enum LaunchError {
    /// Failed to start the agent process.
    #[error("failed to spawn agent: {0}")]
    AgentSpawnError(String),
    /// The agent process exited with a non-zero status.
    #[error("agent exited with non-zero status: {0}")]
    AgentExitError(std::process::ExitStatus),
    /// A Git operation failed during workspace preparation.
    #[error(transparent)]
    Git(#[from] GitError),
}

impl From<TerminalError> for LaunchError {
    fn from(err: TerminalError) -> Self {
        LaunchError::AgentSpawnError(err.to_string())
    }
}

/// Orchestrates the creation of a workspace and the launching of an agent within it.
pub struct AgentLauncher<W, T>
where
    W: WorkspaceFactory,
    T: Terminal,
{
    /// Workspace factory used to create isolated environments.
    workspace: W,
    /// Terminal implementation for tab/split launches.
    terminal: Option<T>,
    /// Selected launch mode for the agent.
    launch_mode: LaunchMode,
    /// Agent definition to execute.
    agent: Agent,
    /// Optional branch name for the new workspace.
    branch: Option<String>,
}

impl<W, T> AgentLauncher<W, T>
where
    W: WorkspaceFactory,
    T: Terminal,
{
    /// Creates a new `AgentLauncher`.
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

    /// Internal method to launch the agent in the specified workspace directory.
    fn launch_in_workspace(&self, workspace_path: &std::path::Path) -> Result<(), LaunchError> {
        let status = self
            .agent
            .process()
            .current_dir(workspace_path)
            .status()
            .map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;

        if status.success() {
            Ok(())
        } else {
            Err(LaunchError::AgentExitError(status))
        }
    }

    /// Executes the launch process: creates the workspace and then launches the agent.
    ///
    /// A `silo.lock` file is created in the workspace before the agent starts.
    /// For [`LaunchMode::ExecReplace`] the lock is removed automatically once
    /// the agent process exits.  For tab/split-pane launches the process runs
    /// detached, so the lock persists until removed manually:
    ///
    /// ```text
    /// rm <workspace>/silo.lock
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if workspace creation, lock acquisition, or
    /// agent spawning fails.
    pub fn launch(&self) -> Result<std::path::PathBuf, LaunchError> {
        // Step 1: Create (or locate) workspace.
        let workspace_path = self.workspace.create(self.branch.clone())?;

        // Step 2: Acquire lock — fails if workspace is already in use.
        let lock = WorkspaceLock::new(&workspace_path);
        lock.try_acquire().map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;

        // Step 3: Launch agent in the workspace.
        match self.launch_mode {
            LaunchMode::ExecReplace => {
                let result = self.launch_in_workspace(&workspace_path);
                // Agent has exited; release the lock so the workspace can be reused.
                lock.release();
                result?;
            }
            // For async launches the agent runs in a detached terminal session.
            // We cannot detect when it exits, so the lock must be removed manually.
            LaunchMode::NewTab => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for new tab".to_string())
                })?;
                match terminal.open_tab(&workspace_path, &self.agent) {
                    Ok(()) => {}
                    Err(_e) => {
                        let result = self.launch_in_workspace(&workspace_path);
                        lock.release();
                        result?;
                    }
                }
            }
            LaunchMode::SplitPane => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError(
                        "no terminal provided for split pane".to_string(),
                    )
                })?;
                match terminal.split_pane(&workspace_path, &self.agent) {
                    Ok(()) => {}
                    Err(_e) => {
                        let result = self.launch_in_workspace(&workspace_path);
                        lock.release();
                        result?;
                    }
                }
            }
        }

        Ok(workspace_path)
    }
}
