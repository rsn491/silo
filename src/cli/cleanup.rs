//! Logic for the `cleanup` command.

use clap::Parser;
use dialoguer::{Select, theme::ColorfulTheme};
use std::collections::HashSet;

use crate::infra::git::GitOperations;
use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::AgentListService;
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::workspace_kind::WorkspaceKind;
use crate::services::workspace_manager::WorkspaceManager;

/// Arguments for the `cleanup` command.
#[derive(Parser, Debug)]
pub struct CleanupArgs {
    /// Clean ALL worktrees in the repo, not just silo-managed ones in `~/.silo/`.
    #[arg(long)]
    pub all: bool,

    /// Force removal even if workspace has uncommitted work.
    #[arg(long)]
    pub force: bool,

    /// Skip confirmation prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Handler for the `cleanup` command.
pub struct CleanupCommand<G: GitOperations, P: ProcessOperations + Clone> {
    /// Workspace manager for both worktrees and checkouts.
    workspaces: GlobalWorkspaceManager<G>,
    /// Service for detecting active agent workspaces.
    agent_list: AgentListService<G, P>,
}

impl<G: GitOperations, P: ProcessOperations + Clone> CleanupCommand<G, P> {
    /// Creates a new `CleanupCommand`.
    pub fn new(workspaces: GlobalWorkspaceManager<G>, agent_list: AgentListService<G, P>) -> Self {
        Self {
            workspaces,
            agent_list,
        }
    }

    /// Executes the cleanup operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails or if user input cannot be read.
    pub fn run(&self, args: CleanupArgs) -> Result<(), Box<dyn std::error::Error>> {
        if !args.yes {
            let confirmed = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("This will remove all inactive worktrees. Continue?")
                .items(["Yes", "No"])
                .default(1)
                .interact()?
                == 0;

            if !confirmed {
                println!("Cleanup cancelled.");
                return Ok(());
            }
            println!();
        }

        let active_paths: HashSet<_> = self
            .agent_list
            .get_active_paths()
            .map_err(|_| "Failed to list running agents")?
            .into_iter()
            .collect();

        let result = self
            .workspaces
            .cleanup(&active_paths, args.all, args.force)?;

        if result.removed.is_empty() && result.failed.is_empty() && result.skipped.is_empty() {
            println!("No workspaces to clean up.");
            return Ok(());
        }

        for ws in &result.removed {
            let detail = match ws.kind {
                WorkspaceKind::Worktree => {
                    format!("branch: {}", ws.branch.as_deref().unwrap_or("(detached)"))
                }
                WorkspaceKind::Clone => "checkout clone".to_string(),
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
                WorkspaceKind::Clone => "checkout clone".to_string(),
            };
            println!(
                "  ⚠ Skipped {} ({}, has {} unpushed commit(s)) — use --force to remove anyway",
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
}
