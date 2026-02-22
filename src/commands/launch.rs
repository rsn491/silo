use clap::Parser;

use crate::infra::agent::Agent;
use crate::infra::git::Git;
use crate::infra::terminal;
use crate::services::agent_launcher::{AgentLauncher, LaunchMode};
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

pub fn run(args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
    let agent = args.agent;

    let (launch_mode, terminal): (LaunchMode, Option<_>) = if args.tab || args.split_pane {
        let term = terminal::detect_terminal()?;
        if args.split_pane {
            (LaunchMode::SplitPane, Some(term))
        } else {
            (LaunchMode::NewTab, Some(term))
        }
    } else {
        (LaunchMode::ExecReplace, None)
    };

    let workspace_kind = if args.checkout {
        "checkout"
    } else {
        "worktree"
    };
    eprintln!("Launching {:?} in {}...", agent, workspace_kind);

    if args.checkout {
        let workspace = GitCheckoutWorkspace::new(Git);
        AgentLauncher::new(workspace, terminal, launch_mode, agent, args.branch).launch()?;
    } else {
        let workspace = GitWorktreeWorkspace::new(Git);
        AgentLauncher::new(workspace, terminal, launch_mode, agent, args.branch).launch()?;
    }

    Ok(())
}
