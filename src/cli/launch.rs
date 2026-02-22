use clap::Parser;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::infra::agent::Agent;
use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::infra::git_error::GitError;
use crate::infra::terminal::Terminal;
use crate::services::agent_launcher::{AgentLauncher, LaunchError, LaunchMode};
use crate::services::agent_workspace::{
    AgentWorkspaceManager, CleanupError, CleanupResult, GitStatus, StatusError,
};
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;

#[derive(Parser, Debug)]
pub struct LaunchArgs {
    /// Custom branch name (default: worktree name)
    #[arg(long)]
    pub branch: Option<String>,

    /// Launch the agent in a new terminal tab instead of replacing the current process
    #[arg(long, group = "windowing")]
    pub tab: bool,

    /// Launch the agent in a vertical split pane
    #[arg(long, group = "windowing")]
    pub split_pane: bool,

    /// Agent command to launch (default: claude)
    #[arg(long, default_value_t = Agent::default())]
    pub agent: Agent,

    /// Use git clone instead of git worktrees for workspace isolation
    #[arg(long)]
    pub checkout: bool,
}

pub enum WorkspaceBackend<G: GitOperations> {
    Worktree(GitWorktreeWorkspace<G>),
    Checkout(GitCheckoutWorkspace<G>),
}

impl<G: GitOperations> AgentWorkspaceManager for WorkspaceBackend<G> {
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.create(branch),
            WorkspaceBackend::Checkout(w) => w.create(branch),
        }
    }

    fn get_statuses(&self, show_all: bool) -> Result<Vec<GitStatus>, StatusError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.get_statuses(show_all),
            WorkspaceBackend::Checkout(w) => w.get_statuses(show_all),
        }
    }

    fn cleanup(
        &self,
        excluded_paths: &HashSet<PathBuf>,
        all: bool,
        force: bool,
    ) -> Result<CleanupResult, CleanupError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.cleanup(excluded_paths, all, force),
            WorkspaceBackend::Checkout(w) => w.cleanup(excluded_paths, all, force),
        }
    }

    fn get_all(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.get_all(),
            WorkspaceBackend::Checkout(w) => w.get_all(),
        }
    }
}

pub struct LaunchCommand<G: GitOperations, T: Terminal> {
    workspace: WorkspaceBackend<G>,
    terminal: Option<T>,
    launch_mode: LaunchMode,
}

impl<G: GitOperations, T: Terminal> LaunchCommand<G, T> {
    pub fn new(
        workspace: WorkspaceBackend<G>,
        terminal: Option<T>,
        launch_mode: LaunchMode,
    ) -> Self {
        Self {
            workspace,
            terminal,
            launch_mode,
        }
    }

    pub fn run(self, args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
        let workspace_kind = match &self.workspace {
            WorkspaceBackend::Worktree(_) => "worktree",
            WorkspaceBackend::Checkout(_) => "checkout",
        };
        eprintln!("Launching {:?} in {}...", args.agent, workspace_kind);

        AgentLauncher::new(
            self.workspace,
            self.terminal,
            self.launch_mode,
            args.agent,
            args.branch,
        )
        .launch()?;

        Ok(())
    }
}
