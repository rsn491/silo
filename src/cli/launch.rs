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
use crate::services::silo_config::SiloConfig;
use crate::services::workspace_kind::WorkspaceKind;

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

    /// Agent command to launch (default: settings.json or claude)
    #[arg(long)]
    pub agent: Option<Agent>,

    /// Use git clone instead of git worktrees for workspace isolation
    #[arg(long, group = "workspace")]
    pub checkout: bool,

    /// Use git worktrees for workspace isolation (overrides settings.json default)
    #[arg(long, group = "workspace")]
    pub worktree: bool,
}

enum WorkspaceBackend<G: GitOperations> {
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
    git: G,
    terminal: Option<T>,
    launch_mode: LaunchMode,
}

impl<G: GitOperations, T: Terminal> LaunchCommand<G, T> {
    pub fn new(git: G, terminal: Option<T>, launch_mode: LaunchMode) -> Self {
        Self {
            git,
            terminal,
            launch_mode,
        }
    }

    pub fn run(self, args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
        let agent = resolve_agent(args.agent);
        let workspace = match resolve_workspace_type(args.checkout, args.worktree) {
            WorkspaceKind::Checkout => {
                WorkspaceBackend::Checkout(GitCheckoutWorkspace::new(self.git))
            }
            WorkspaceKind::Worktree => {
                WorkspaceBackend::Worktree(GitWorktreeWorkspace::new(self.git))
            }
        };
        let workspace_kind = match &workspace {
            WorkspaceBackend::Worktree(_) => "worktree",
            WorkspaceBackend::Checkout(_) => "checkout",
        };
        eprintln!("Launching {:?} in {}...", agent, workspace_kind);

        let workspace_path = AgentLauncher::new(
            workspace,
            self.terminal,
            self.launch_mode,
            agent,
            args.branch,
        )
        .launch()?;

        eprintln!(
            "\n\nAgent exited. To resume, cd to the workspace:\n  cd {}",
            workspace_path.display()
        );

        Ok(())
    }
}

fn resolve_workspace_type(checkout: bool, worktree: bool) -> WorkspaceKind {
    if checkout {
        return WorkspaceKind::Checkout;
    }
    if worktree {
        return WorkspaceKind::Worktree;
    }

    let settings = match SiloConfig::load_settings() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Warning: failed to load settings.json: {}", err);
            return WorkspaceKind::Worktree;
        }
    };

    settings.workspace_type.unwrap_or(WorkspaceKind::Worktree)
}

fn resolve_agent(agent: Option<Agent>) -> Agent {
    if let Some(agent) = agent {
        return agent;
    }

    let settings = match SiloConfig::load_settings() {
        Ok(settings) => settings,
        Err(err) => {
            eprintln!("Warning: failed to load settings.json: {}", err);
            return Agent::default();
        }
    };

    let Some(agent_str) = settings.agent else {
        return Agent::default();
    };

    match Agent::try_from_str(&agent_str) {
        Some(agent) => agent,
        None => {
            eprintln!(
                "Warning: invalid agent '{}' in settings.json; falling back to default",
                agent_str
            );
            Agent::default()
        }
    }
}
