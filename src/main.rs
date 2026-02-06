mod infra;
mod services;

use clap::{Parser, Subcommand};
use infra::git::Git;
use services::agent_launcher::AgentLauncher;
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Launch(args) => {
            GitWorktreeLauncher::new(Git::default(), args.worktree_base, args.branch).launch()?;
        }
    };

    Ok(())
}
