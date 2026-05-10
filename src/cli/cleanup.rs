//! Logic for the `cleanup` command.

use std::collections::HashSet;

use clap::Parser;
use crossterm::style::Color;

use crate::infra::git::GitOperations;
use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::AgentListService;
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::workspace_kind::WorkspaceKind;
use crate::services::workspace_manager::WorkspaceManager;
use crate::tui::{print_status, run_confirm};

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
            let confirmed = run_confirm(
                "This will remove all inactive worktrees. Continue?",
                false,
            )?;

            if !confirmed {
                print_status("✗", Color::DarkGrey, "Cleanup cancelled.")?;
                return Ok(());
            }
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
            print_status("–", Color::DarkGrey, "No workspaces to clean up.")?;
            return Ok(());
        }

        for ws in &result.removed {
            let detail = workspace_detail(&ws.kind, ws.branch.as_deref());
            print_status(
                "✓",
                Color::Green,
                &format!("Removed {}  ({})", ws.path.display(), detail),
            )?;
        }
        for ws in &result.failed {
            print_status(
                "✗",
                Color::Red,
                &format!("Failed to remove {}: {}", ws.path.display(), ws.error),
            )?;
        }
        for ws in &result.skipped {
            let detail = workspace_detail(&ws.kind, ws.branch.as_deref());
            print_status(
                "⚠",
                Color::Yellow,
                &format!(
                    "Skipped {}  ({}, {} unpushed commit(s)) — use --force to remove anyway",
                    ws.path.display(),
                    detail,
                    ws.commits_ahead,
                ),
            )?;
        }

        // Summary line
        let mut parts = vec![format!("{} removed", result.removed.len())];
        if !result.failed.is_empty() {
            parts.push(format!("{} failed", result.failed.len()));
        }
        if !result.skipped.is_empty() {
            parts.push(format!("{} skipped", result.skipped.len()));
        }
        print_status("→", Color::Cyan, &parts.join(" · "))?;

        Ok(())
    }
}

fn workspace_detail(kind: &WorkspaceKind, branch: Option<&str>) -> String {
    match kind {
        WorkspaceKind::Worktree => {
            format!("branch: {}", branch.unwrap_or("(detached)"))
        }
        WorkspaceKind::Clone => "checkout clone".to_string(),
    }
}
