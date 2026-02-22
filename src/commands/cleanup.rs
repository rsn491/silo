use clap::Parser;
use std::io::{self, Write};

use crate::infra::git::Git;
use crate::infra::process::SystemProcess;
use crate::services::agent_workspace::WorkspaceKind;
use crate::services::workspace_cleanup::WorkspaceCleanupService;

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

    let service = WorkspaceCleanupService::new(Git, SystemProcess);
    let result = service.cleanup(args.all)?;

    if result.removed.is_empty() && result.failed.is_empty() && result.skipped.is_empty() {
        println!("No workspaces to clean up.");
        return Ok(());
    }

    for ws in &result.removed {
        let detail = match ws.kind {
            WorkspaceKind::Worktree => {
                format!("branch: {}", ws.branch.as_deref().unwrap_or("(detached)"))
            }
            WorkspaceKind::Checkout => "checkout clone".to_string(),
        };
        println!("  ✓ Removed {} ({})", ws.path.display(), detail);
    }
    for ws in &result.failed {
        println!("  ✗ Failed to remove {}: {}", ws.path.display(), ws.error);
    }
    for ws in &result.skipped {
        let detail = match ws.kind {
            WorkspaceKind::Worktree => {
                format!("branch: {}", ws.branch.as_deref().unwrap_or("(detached)"))
            }
            WorkspaceKind::Checkout => "checkout clone".to_string(),
        };
        println!(
            "  ⚠ Skipped {} ({}, has {} unpushed commit(s))",
            ws.path.display(),
            detail,
            ws.commits_ahead
        );
    }

    println!("Successfully removed {} workspace(s)", result.removed.len());
    if !result.failed.is_empty() {
        println!("Failed to remove {} workspace(s)", result.failed.len());
    }
    if !result.skipped.is_empty() {
        println!(
            "Skipped {} workspace(s) with unpushed commits",
            result.skipped.len()
        );
    }

    Ok(())
}
