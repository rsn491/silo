use clap::Parser;

use crate::infra::git::Git;
use crate::services::agent_workspace::AgentWorkspaceManager;
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;

#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Show all workspaces, including clean ones
    #[arg(long)]
    pub all: bool,
}

pub fn run(args: StatusArgs) -> Result<(), Box<dyn std::error::Error>> {
    let worktree_workspace = GitWorktreeWorkspace::new(Git, None);
    let mut statuses = worktree_workspace.get_statuses(args.all)?;

    let checkout_workspace = GitCheckoutWorkspace::new(Git);
    statuses.extend(checkout_workspace.get_statuses(args.all)?);

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
