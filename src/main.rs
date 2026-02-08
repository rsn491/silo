mod infra;
mod services;

use clap::{Parser, Subcommand};
use infra::git::Git;
use infra::process::SystemProcess;
use infra::terminal::{self, Terminal};
use services::agent_launcher::{AgentLauncher, LaunchMode};
use services::agent_list::AgentListService;
use services::git_worktree_launcher::GitWorktreeLauncher;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "silo")]
#[command(about = "A CLI tool for managing isolated Claude workspaces")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new git worktree and launch Claude in it
    Launch(LaunchArgs),
    /// List running agents in worktrees of the current repository
    Ps,
}

#[derive(Parser, Debug)]
pub struct LaunchArgs {
    /// Base directory for the worktree (default: parent of repo)
    #[arg(long)]
    pub worktree_base: Option<PathBuf>,

    /// Custom branch name (default: worktree name)
    #[arg(long)]
    pub branch: Option<String>,

    /// Launch the agent in a new terminal tab instead of replacing the current process
    #[arg(long, group = "windowing")]
    pub tab: bool,

    /// Launch the agent in a vertical split pane
    #[arg(long, group = "windowing")]
    pub split_pane: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Launch(args) => {
            let (launch_mode, terminal): (LaunchMode, Option<Box<dyn Terminal>>) =
                if args.tab || args.split_pane {
                    let term = terminal::detect_terminal()?;
                    if args.split_pane {
                        (LaunchMode::SplitPane, Some(term))
                    } else {
                        (LaunchMode::NewTab, Some(term))
                    }
                } else {
                    (LaunchMode::ExecReplace, None)
                };

            GitWorktreeLauncher::new(Git, args.worktree_base, args.branch, terminal)
                .launch(launch_mode)?;
        }
        Commands::Ps => {
            let agents = AgentListService::new(Git, SystemProcess).list_running_agents()?;

            if agents.is_empty() {
                println!("No running agents found in this repository's worktrees.");
            } else {
                println!("{:<8} {:<10} {:<20} WORKTREE", "PID", "AGENT", "BRANCH");
                for agent in &agents {
                    println!(
                        "{:<8} {:<10} {:<20} {}",
                        agent.pid,
                        agent.agent_type,
                        agent.branch.as_deref().unwrap_or("(detached)"),
                        agent.worktree_path.display()
                    );
                }
            }
        }
    };

    Ok(())
}
