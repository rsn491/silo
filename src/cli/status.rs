//! Logic for the `status` command.

use crate::infra::git::GitOperations;
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::workspace_manager::WorkspaceManager;

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
        if statuses.is_empty() {
            println!("No workspaces found (excluding main worktree).");
            return Ok(());
        }

        println!("{:<32} {:<32} LATEST COMMIT", "ID", "BRANCH");

        for status in &statuses {
            let id = status
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let branch = status.branch.as_deref().unwrap_or("(detached)");
            let commit = status.latest_commit.as_deref().unwrap_or("-");

            println!(
                "{:<32} {:<32} {}",
                trunc(id, 32),
                trunc(branch, 32),
                trunc(commit, 50)
            );
        }

        Ok(())
    }
}

/// Truncates a string to the specified maximum length, adding an ellipsis if needed.
fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}
