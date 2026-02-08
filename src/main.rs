mod infra;
mod services;

use clap::{Parser, Subcommand};
use infra::git::Git;
use infra::terminal::{self, Terminal};
use services::agent_launcher::{AgentLauncher, LaunchMode};
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
    };

    Ok(())
}
