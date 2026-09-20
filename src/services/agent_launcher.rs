//! Logic for launching AI agents in isolated workspaces.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use crate::infra::agent::Agent;
use crate::infra::git_error::GitError;
use crate::infra::terminal::{Terminal, TerminalError};
use crate::services::workspace_lock::{WorkspaceLock, WorkspaceLockGuard};
use crate::services::workspace_manager::WorkspaceFactory;
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

/// Workspace selection inputs for an agent launch.
pub struct WorkspaceLaunchOptions {
    /// Optional branch name for the new workspace.
    pub branch: Option<String>,
    /// Whether to reuse an existing inactive workspace.
    pub reuse: bool,
    /// Workspaces where process discovery found a running agent.
    pub active_paths: HashSet<PathBuf>,
    /// When set, skip workspace creation and launch directly in this path.
    pub override_path: Option<PathBuf>,
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
    /// Whether to reuse an existing inactive workspace.
    reuse: bool,
    /// Workspaces where process discovery found a running agent.
    active_paths: HashSet<PathBuf>,
    /// When set, skip workspace creation and launch directly in this path.
    override_path: Option<PathBuf>,
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
        options: WorkspaceLaunchOptions,
    ) -> Self {
        Self {
            workspace,
            terminal,
            launch_mode,
            agent,
            branch: options.branch,
            reuse: options.reuse,
            active_paths: options.active_paths,
            override_path: options.override_path,
        }
    }

    /// Internal method to launch the agent in the specified workspace directory.
    fn launch_in_workspace(
        &self,
        workspace_path: &std::path::Path,
        lock: WorkspaceLockGuard,
    ) -> Result<(), LaunchError> {
        let command = self.agent.process();
        launch_process(command, workspace_path, lock)
    }

    /// Executes the launch process: creates the workspace and then launches the agent.
    ///
    /// A `silo.lock` file serializes the short launch operation. It is removed
    /// as soon as the agent process is spawned or a terminal accepts the launch
    /// command, and on every error path.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if workspace creation, lock acquisition, or
    /// agent spawning fails.
    pub fn launch(&self) -> Result<std::path::PathBuf, LaunchError> {
        // Step 1: Create (or locate) workspace.
        let workspace_path = if let Some(path) = self.override_path.clone() {
            path
        } else {
            self.workspace
                .create(self.branch.clone(), self.reuse, &self.active_paths)?
        };

        // Step 2: Acquire lock — fails if another launch is in progress.
        let lock = WorkspaceLock::new(&workspace_path)
            .try_acquire()
            .map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;

        // Step 3: Launch agent in the workspace.
        match self.launch_mode {
            LaunchMode::ExecReplace => {
                self.launch_in_workspace(&workspace_path, lock)?;
            }
            LaunchMode::NewTab => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for new tab".to_string())
                })?;
                match terminal.open_tab(&workspace_path, &self.agent) {
                    Ok(()) => drop(lock),
                    Err(_e) => {
                        self.launch_in_workspace(&workspace_path, lock)?;
                    }
                }
            }
            LaunchMode::SplitPane => {
                // SplitPane is handled before AgentLauncher is created (in LaunchCommand::run).
                unreachable!("SplitPane should be handled upstream in LaunchCommand");
            }
        }

        Ok(workspace_path)
    }
}

/// Spawns an agent command, releases the launch mutex, and waits for exit.
fn launch_process(
    mut command: Command,
    workspace_path: &std::path::Path,
    lock: WorkspaceLockGuard,
) -> Result<(), LaunchError> {
    let mut child = command
        .current_dir(workspace_path)
        .spawn()
        .map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;
    drop(lock);

    let status = child
        .wait()
        .map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;

    if status.success() {
        Ok(())
    } else {
        Err(LaunchError::AgentExitError(status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::terminal::TerminalError;
    use std::path::Path;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct SuccessfulTerminal;

    impl Terminal for SuccessfulTerminal {
        fn open_tab(&self, _path: &Path, _agent: &Agent) -> Result<(), TerminalError> {
            Ok(())
        }

        fn split_pane(&self, _path: &Path, _command: &str) -> Result<(), TerminalError> {
            unreachable!()
        }
    }

    struct ExistingWorkspace(PathBuf);

    impl WorkspaceFactory for ExistingWorkspace {
        fn create(
            &self,
            _branch: Option<String>,
            _reuse: bool,
            _active_paths: &HashSet<PathBuf>,
        ) -> Result<PathBuf, LaunchError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn successful_tab_dispatch_releases_launch_lock() {
        let dir = TempDir::new().unwrap();
        let launcher = AgentLauncher::new(
            ExistingWorkspace(dir.path().to_path_buf()),
            Some(SuccessfulTerminal),
            LaunchMode::NewTab,
            Agent::Codex,
            WorkspaceLaunchOptions {
                branch: None,
                reuse: false,
                active_paths: HashSet::new(),
                override_path: None,
            },
        );

        launcher.launch().unwrap();

        assert!(!dir.path().join("silo.lock").exists());
    }

    #[test]
    fn missing_terminal_releases_launch_lock() {
        let dir = TempDir::new().unwrap();
        let launcher = AgentLauncher::new(
            ExistingWorkspace(dir.path().to_path_buf()),
            None::<SuccessfulTerminal>,
            LaunchMode::NewTab,
            Agent::Codex,
            WorkspaceLaunchOptions {
                branch: None,
                reuse: false,
                active_paths: HashSet::new(),
                override_path: None,
            },
        );

        assert!(launcher.launch().is_err());
        assert!(!dir.path().join("silo.lock").exists());
    }

    #[test]
    fn spawn_failure_releases_launch_lock() {
        let dir = TempDir::new().unwrap();
        let guard = WorkspaceLock::new(dir.path()).try_acquire().unwrap();
        let command = Command::new("/definitely/not/a/silo-agent");

        assert!(launch_process(command, dir.path(), guard).is_err());
        assert!(!dir.path().join("silo.lock").exists());
    }

    #[test]
    #[cfg(unix)]
    fn direct_launch_releases_lock_before_waiting() {
        let dir = TempDir::new().unwrap();
        let guard = WorkspaceLock::new(dir.path()).try_acquire().unwrap();
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "i=0; while test -e silo.lock && test $i -lt 100; do i=$((i + 1)); sleep 0.01; done; test ! -e silo.lock",
        ]);

        launch_process(command, dir.path(), guard).unwrap();

        assert!(!dir.path().join("silo.lock").exists());
    }
}
