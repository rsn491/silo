//! Logic for the `cleanup` command.

use std::collections::HashSet;

use clap::Parser;
use crossterm::style::Color;

use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::AgentListService;
use crate::services::workspace_kind::WorkspaceKind;
use crate::services::workspace_manager::WorkspaceManager;
use crate::tui::{Prompt, print_status};

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
pub struct CleanupCommand<W: WorkspaceManager, L: WorkspaceManager, P: ProcessOperations, U: Prompt>
{
    /// Workspace manager for both worktrees and checkouts.
    workspaces: W,
    /// Service for detecting active agent workspaces.
    agent_list: AgentListService<L, P>,
    /// Interactive prompt used for confirmation.
    prompt: U,
}

impl<W: WorkspaceManager, L: WorkspaceManager, P: ProcessOperations, U: Prompt>
    CleanupCommand<W, L, P, U>
{
    /// Creates a new `CleanupCommand`.
    pub fn new(workspaces: W, agent_list: AgentListService<L, P>, prompt: U) -> Self {
        Self {
            workspaces,
            agent_list,
            prompt,
        }
    }

    /// Executes the cleanup operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails or if user input cannot be read.
    pub fn run(&self, args: CleanupArgs) -> Result<(), Box<dyn std::error::Error>> {
        if !args.yes {
            let confirmed = self
                .prompt
                .confirm("This will remove all inactive worktrees. Continue?", false)?;

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

/// Formats a human-readable detail string for a workspace (branch or clone label).
fn workspace_detail(kind: &WorkspaceKind, branch: Option<&str>) -> String {
    match kind {
        WorkspaceKind::Worktree => {
            format!("branch: {}", branch.unwrap_or("(detached)"))
        }
        WorkspaceKind::Clone => "checkout clone".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::infra::git_error::GitError;
    use crate::infra::system_process::{MockProcessOperations, ProcessError};
    use crate::services::workspace_manager::{
        CleanupError, CleanupResult, FailedWorkspace, MockWorkspaceManager, RemovedWorkspace,
        SkippedWorkspace,
    };
    use crate::tui::prompt::MockPrompt;

    /// An agent list service that reports no running agents.
    fn idle_agents() -> AgentListService<MockWorkspaceManager, MockProcessOperations> {
        let mut listing = MockWorkspaceManager::new();
        listing.expect_get_all().returning(|_| Ok(vec![]));
        let mut process = MockProcessOperations::new();
        process
            .expect_find_processes_by_names()
            .returning(|_| Ok(vec![]));
        AgentListService::new(listing, process)
    }

    /// Default args with confirmation skipped.
    fn yes_args() -> CleanupArgs {
        CleanupArgs {
            all: false,
            force: false,
            yes: true,
        }
    }

    #[test]
    fn declined_confirmation_cancels_cleanup() {
        // Given
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().times(1).returning(|_, _| Ok(false));
        let mut workspaces = MockWorkspaceManager::new();
        workspaces.expect_cleanup().never();
        let args = CleanupArgs {
            yes: false,
            ..yes_args()
        };

        // When
        CleanupCommand::new(workspaces, idle_agents(), prompt)
            .run(args)
            .unwrap();

        // Then
        // completing without error is the assertion
    }

    #[test]
    fn confirmed_cleanup_reports_nothing_to_do() {
        // Given
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().times(1).returning(|_, _| Ok(true));
        let mut workspaces = MockWorkspaceManager::new();
        workspaces
            .expect_cleanup()
            .times(1)
            .returning(|_, _, _| Ok(CleanupResult::default()));
        let args = CleanupArgs {
            yes: false,
            ..yes_args()
        };

        // When
        CleanupCommand::new(workspaces, idle_agents(), prompt)
            .run(args)
            .unwrap();

        // Then
        // completing without error is the assertion
    }

    #[test]
    fn reports_removed_failed_and_skipped_workspaces() {
        // Given
        let mut workspaces = MockWorkspaceManager::new();
        workspaces
            .expect_cleanup()
            .withf(|excluded, all, force| excluded.is_empty() && *all && *force)
            .returning(|_, _, _| {
                Ok(CleanupResult {
                    removed: vec![
                        RemovedWorkspace {
                            path: PathBuf::from("/silo/a"),
                            kind: WorkspaceKind::Worktree,
                            branch: Some("feature".to_string()),
                        },
                        RemovedWorkspace {
                            path: PathBuf::from("/silo/b"),
                            kind: WorkspaceKind::Clone,
                            branch: None,
                        },
                    ],
                    failed: vec![FailedWorkspace {
                        path: PathBuf::from("/silo/c"),
                        error: "busy".to_string(),
                    }],
                    skipped: vec![SkippedWorkspace {
                        path: PathBuf::from("/silo/d"),
                        kind: WorkspaceKind::Worktree,
                        branch: None,
                        commits_ahead: 2,
                    }],
                })
            });
        let args = CleanupArgs {
            all: true,
            force: true,
            yes: true,
        };

        // When
        CleanupCommand::new(workspaces, idle_agents(), MockPrompt::new())
            .run(args)
            .unwrap();

        // Then
        // completing without error is the assertion
    }

    #[test]
    fn fails_when_agent_listing_fails() {
        // Given
        let mut listing = MockWorkspaceManager::new();
        listing.expect_get_all().returning(|_| Ok(vec![]));
        let mut process = MockProcessOperations::new();
        process
            .expect_find_processes_by_names()
            .returning(|_| Err(ProcessError::CommandFailed("ps missing".to_string())));
        let mut workspaces = MockWorkspaceManager::new();
        workspaces.expect_cleanup().never();
        let result = CleanupCommand::new(
            workspaces,
            AgentListService::new(listing, process),
            MockPrompt::new(),
        )
        .run(yes_args());

        // When
        let actual = result.unwrap_err().to_string();

        // Then
        assert_eq!(actual, "Failed to list running agents");
    }

    #[test]
    fn propagates_cleanup_errors() {
        // Given
        let mut workspaces = MockWorkspaceManager::new();
        workspaces
            .expect_cleanup()
            .returning(|_, _, _| Err(CleanupError::Git(GitError::NotAGitRepo)));

        // When
        let result =
            CleanupCommand::new(workspaces, idle_agents(), MockPrompt::new()).run(yes_args());

        // Then
        assert!(result.is_err());
    }

    #[test]
    fn workspace_detail_describes_kind() {
        // Given
        // (no setup needed)

        // When
        let actual_1 = workspace_detail(&WorkspaceKind::Worktree, Some("main"));
        let actual_2 = workspace_detail(&WorkspaceKind::Worktree, None);
        let actual_3 = workspace_detail(&WorkspaceKind::Clone, Some("main"));

        // Then
        assert_eq!(actual_1, "branch: main");
        assert_eq!(actual_2, "branch: (detached)");
        assert_eq!(actual_3, "checkout clone");
    }
}
