use clap::Parser;
use std::io::{self, Write};

use crate::infra::git::Git;
use crate::infra::process::SystemProcess;
use crate::services::worktree_cleanup::WorktreeCleanupService;

#[derive(Parser, Debug)]
pub struct CleanupArgs {
    /// Clean ALL worktrees in the repo, not just silo-managed ones in ~/.silo/
    #[arg(long)]
    pub all: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

pub fn run(args: CleanupArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.yes {
        print!("This will remove all inactive worktrees. Continue? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Cleanup cancelled.");
            return Ok(());
        }
        println!();
    }

    let service = WorktreeCleanupService::new(Git, SystemProcess);
    service.cleanup(args.all)?;

    Ok(())
}
