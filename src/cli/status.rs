//! Logic for the `status` command.

use clap::Parser;

use crate::infra::git::GitOperations;
use crate::services::agent_workspace::WorkspaceManager;
use crate::services::global_workspace::GlobalWorkspaceManager;

/// Arguments for the `status` command.
#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Show all workspaces, including clean ones.
    #[arg(long)]
    pub all: bool,
}

/// Handler for the `status` command.
pub struct StatusCommand<G: GitOperations> {
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
    pub fn run(&self, args: StatusArgs) -> Result<(), Box<dyn std::error::Error>> {
        let workspaces = self.workspace_manager.get_all()?;
        let statuses: Vec<_> = workspaces
            .into_iter()
            .filter(|w| {
                args.all || w.has_uncommitted_changes || w.commits_ahead > 0 || w.commits_behind > 0
            })
            .collect();
        if statuses.is_empty() {
            if args.all {
                println!("No workspaces found (excluding main worktree).");
            } else {
                println!("No workspaces with changes or commits ahead/behind.");
                println!("Use --all to see all workspaces.");
            }
            return Ok(());
        }

        println!(
            "{:<10} {:<50} {:<20} {:<12} {:<12}",
            "TYPE", "PATH", "BRANCH", "UNCOMMITTED", "AHEAD/BEHIND"
        );

        for status in &statuses {
            let branch = status.branch.as_deref().unwrap_or("(detached)");

            let uncommitted = if status.has_uncommitted_changes {
                format!("{} files", status.uncommitted_file_count)
            } else {
                "-".to_string()
            };

            let divergence = if status.commits_ahead > 0 || status.commits_behind > 0 {
                format!("+{} -{}", status.commits_ahead, status.commits_behind)
            } else {
                "-".to_string()
            };

            println!(
                "{:<10} {:<50} {:<20} {:<12} {:<12}",
                status.kind,
                status.path.display(),
                branch,
                uncommitted,
                divergence
            );
        }

        Ok(())
    }
}
