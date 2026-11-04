//! Logic for the `status` command.

use std::io::{self, Write};

use crossterm::style::Color;

use crate::services::workspace_manager::{WorkspaceListingScope, WorkspaceManager};
use crate::tui::{Column, Row, StyledCell, render_table};

/// Handler for the `status` command.
pub struct StatusCommand<W: WorkspaceManager> {
    /// Workspace manager for worktree and checkout status.
    workspace_manager: W,
}

impl<W: WorkspaceManager> StatusCommand<W> {
    /// Creates a new `StatusCommand`.
    pub fn new(workspace_manager: W) -> Self {
        Self { workspace_manager }
    }

    /// Executes the status command to show workspace information.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace status cannot be retrieved.
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.run_to(&mut io::stdout())
    }

    /// Writes the workspace status table to `out`.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace status cannot be retrieved or writing fails.
    fn run_to(&self, out: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
        let statuses = self
            .workspace_manager
            .get_all(WorkspaceListingScope::ManagedAndOriginal)?;

        let cols = vec![
            Column {
                header: "ID",
                width: 24,
            },
            Column {
                header: "BRANCH",
                width: 28,
            },
            Column {
                header: "COMMITS",
                width: 7,
            },
            Column {
                header: "CHANGES",
                width: 7,
            },
            Column {
                header: "LATEST COMMIT",
                width: 48,
            },
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

        render_table(out, "Silo Workspaces", &cols, &rows, "No workspaces found.")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::infra::git::GitWorkspaceInfo;
    use crate::infra::git_error::GitError;
    use crate::services::workspace_manager::MockWorkspaceManager;

    /// Runs the status command against `manager` and returns the rendered output.
    fn render(manager: MockWorkspaceManager) -> Result<String, Box<dyn std::error::Error>> {
        let mut out = Vec::new();
        StatusCommand::new(manager).run_to(&mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    #[test]
    fn renders_workspace_rows() {
        // Given
        let mut manager = MockWorkspaceManager::new();
        manager
            .expect_get_all()
            .withf(|scope| *scope == WorkspaceListingScope::ManagedAndOriginal)
            .returning(|_| {
                Ok(vec![
                    GitWorkspaceInfo {
                        path: PathBuf::from("/repo/original"),
                        branch: Some("main".to_string()),
                        latest_commit: Some("abc123 initial".to_string()),
                        ..Default::default()
                    },
                    GitWorkspaceInfo {
                        path: PathBuf::from("/silo/original-1234abcd"),
                        branch: None,
                        has_uncommitted_changes: true,
                        commits_ahead: 3,
                        latest_commit: None,
                    },
                ])
            });
        // When
        let output = render(manager).unwrap();
        // Then
        assert!(output.contains("Silo Workspaces"));
        assert!(output.contains("original-1234abcd"));
        assert!(output.contains("(detached)"));
        assert!(output.contains("+3"));
        assert!(output.contains("abc123 initial"));
        assert!(output.contains('*'));
    }

    #[test]
    fn renders_empty_message() {
        // Given
        let mut manager = MockWorkspaceManager::new();
        manager.expect_get_all().returning(|_| Ok(vec![]));
        // When
        let output = render(manager).unwrap();
        // Then
        assert!(output.contains("No workspaces found."));
    }

    #[test]
    fn propagates_listing_errors() {
        // Given
        let mut manager = MockWorkspaceManager::new();
        // When
        manager
            .expect_get_all()
            .returning(|_| Err(GitError::NotAGitRepo));
        // Then
        assert!(render(manager).is_err());
    }
}
