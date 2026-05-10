//! Logic for the `status` command.

use crossterm::style::Color;

use crate::infra::git::GitOperations;
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::workspace_manager::WorkspaceManager;
use crate::tui::{Column, Row, StyledCell, render_table};

/// Handler for the `status` command.
pub struct StatusCommand<G: GitOperations> {
    /// Workspace manager for worktree and checkout status.
    workspace_manager: GlobalWorkspaceManager<G>,
}

impl<G: GitOperations> StatusCommand<G> {
    /// Creates a new `StatusCommand`.
    pub fn new(workspace_manager: GlobalWorkspaceManager<G>) -> Self {
        Self { workspace_manager }
    }

    /// Executes the status command to show workspace information.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace status cannot be retrieved.
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let statuses = self.workspace_manager.get_all()?;

        let cols = vec![
            Column { header: "ID", width: 24 },
            Column { header: "BRANCH", width: 28 },
            Column { header: "COMMITS", width: 7 },
            Column { header: "CHANGES", width: 7 },
            Column { header: "LATEST COMMIT", width: 48 },
        ];

        let rows: Vec<Row> = statuses
            .iter()
            .map(|s| {
                let id = s
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let branch = s.branch.as_deref().unwrap_or("(detached)").to_string();
                let commit = s.latest_commit.as_deref().unwrap_or("—").to_string();

                let commits_cell = if s.commits_ahead == 0 {
                    StyledCell::dim("—")
                } else {
                    StyledCell::colored(format!("+{}", s.commits_ahead), Color::Blue)
                };

                let changes_cell = if s.has_uncommitted_changes {
                    StyledCell::warning("*")
                } else {
                    StyledCell::dim("—")
                };

                Row {
                    cells: vec![
                        StyledCell::plain(id),
                        StyledCell::plain(branch),
                        commits_cell,
                        changes_cell,
                        StyledCell::dim(commit),
                    ],
                }
            })
            .collect();

        render_table("Silo Workspaces", &cols, &rows, "No workspaces found (excluding main worktree).")
    }
}
