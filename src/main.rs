mod commands;
mod infra;
mod services;

use clap::{Parser, Subcommand};
use commands::cleanup::CleanupArgs;
use commands::launch::LaunchArgs;
use commands::status::StatusArgs;

#[derive(Parser)]
#[command(name = "silo")]
#[command(about = "A CLI tool for managing isolated Claude workspaces")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new git worktree and launch an agent in it
    Launch(LaunchArgs),
    /// List running agents in worktrees of the current repository
    Ps,
    /// Initialize the .silo directory in your home directory
    Init,
    /// Clean up worktrees where no agents are running
    Cleanup(CleanupArgs),
    /// Show status of worktrees (uncommitted changes and commits ahead/behind)
    Status(StatusArgs),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Launch(args) => commands::launch::run(args)?,
        Commands::Ps => commands::ps::run()?,
        Commands::Init => commands::init::run()?,
        Commands::Cleanup(args) => commands::cleanup::run(args)?,
        Commands::Status(args) => commands::status::run(args)?,
    }

    Ok(())
}
