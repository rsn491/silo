//! Logic for the `checkout` command.

use std::path::PathBuf;

use clap::Parser;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::workspace_manager::WorkspaceManager;
use crate::tui::{SelectItem, pad_or_trunc, run_select};

/// Arguments for the `checkout` command.
#[derive(Parser, Debug)]
pub struct CheckoutArgs {
    /// The workspace ID (directory name) or branch name to switch into.
    ///
    /// If omitted, an interactive selector is displayed.
    pub workspace_id: Option<String>,
}

/// Handler for the `checkout` command.
pub struct CheckoutCommand<G: GitOperations> {
    /// Workspace manager used to list available workspaces.
    workspace_manager: GlobalWorkspaceManager<G>,
}

impl<G: GitOperations> CheckoutCommand<G> {
    /// Creates a new `CheckoutCommand`.
    pub fn new(workspace_manager: GlobalWorkspaceManager<G>) -> Self {
        Self { workspace_manager }
    }

    /// Executes the checkout command.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace listing, user selection, or shell spawning fails.
    pub fn run(self, args: CheckoutArgs) -> Result<(), Box<dyn std::error::Error>> {
        let workspaces = self.workspace_manager.get_all()?;

        if workspaces.is_empty() {
            eprintln!("No workspaces found.");
            std::process::exit(1);
        }

        let workspace_path = if let Some(id) = args.workspace_id {
            find_by_id(&workspaces, &id)?
        } else {
            select_interactively(&workspaces)?
        };

        spawn_shell_in(&workspace_path)
    }
}

/// Finds a workspace by matching the provided query against each workspace's directory name or
/// branch name, whichever matches first.
///
/// # Errors
///
/// Exits with code 1 if no workspace with the given ID or branch is found.
fn find_by_id(
    workspaces: &[GitWorkspaceInfo],
    id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let found = workspaces.iter().find(|w| {
        let name_match = w.path.file_name().and_then(|n| n.to_str()) == Some(id);
        let branch_match = w.branch.as_deref() == Some(id);
        name_match || branch_match
    });

    match found {
        Some(w) => Ok(w.path.clone()),
        None => {
            eprintln!("No workspace found with ID or branch '{}'.", id);
            std::process::exit(1);
        }
    }
}

/// Presents an interactive ratatui list of workspaces and returns the selected path.
///
/// # Errors
///
/// Returns an error if the terminal interaction fails. Exits with code 1 if the user cancels.
fn select_interactively(
    workspaces: &[GitWorkspaceInfo],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let items: Vec<SelectItem> = workspaces
        .iter()
        .map(|w| {
            let id = w.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let branch = w.branch.as_deref().unwrap_or("(detached)");
            let commit = w.latest_commit.as_deref().unwrap_or("—");
            let label = format!(
                "{}  {}  {}",
                pad_or_trunc(id, 24),
                pad_or_trunc(branch, 28),
                pad_or_trunc(commit, 40),
            );
            let mut detail_parts: Vec<String> = vec![];
            if w.has_uncommitted_changes {
                detail_parts.push("uncommitted changes".to_string());
            }
            if w.commits_ahead > 0 {
                detail_parts.push(format!("{} commit(s) ahead", w.commits_ahead));
            }
            let detail = if detail_parts.is_empty() {
                None
            } else {
                Some(detail_parts.join(" · "))
            };
            SelectItem { label, detail, icon: None }
        })
        .collect();

    match run_select("Select a workspace", &items, 0)? {
        Some(idx) => Ok(workspaces[idx].path.clone()),
        None => {
            eprintln!("No workspace selected.");
            std::process::exit(1);
        }
    }
}

/// Spawns the user's shell with its working directory set to the given workspace path.
///
/// # Errors
///
/// Returns an error if the shell process cannot be spawned.
fn spawn_shell_in(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{
        execute,
        style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    };
    let mut out = std::io::stderr();
    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("Switching to workspace: "),
        ResetColor,
        SetAttribute(Attribute::Reset),
        Print(path.display().to_string()),
        Print("\n"),
    )?;
    execute!(
        out,
        SetForegroundColor(Color::DarkGrey),
        Print("Type 'exit' to return to your previous session.\n"),
        ResetColor,
    )?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let status = std::process::Command::new(&shell)
        .current_dir(path)
        .status()?;

    if let Some(code) = status.code()
        && code != 0
    {
        std::process::exit(code);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::infra::git::GitWorkspaceInfo;

    use super::find_by_id;

    fn make_workspace(dir: &str, branch: Option<&str>) -> GitWorkspaceInfo {
        GitWorkspaceInfo {
            path: PathBuf::from(format!("/silo/{}", dir)),
            branch: branch.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn find_by_id_matches_directory_name() {
        let workspaces = vec![
            make_workspace("project-abc12345", Some("main")),
            make_workspace("project-xyz67890", Some("feature/foo")),
        ];
        let result = find_by_id(&workspaces, "project-abc12345").unwrap();
        assert_eq!(result, PathBuf::from("/silo/project-abc12345"));
    }

    #[test]
    fn find_by_id_matches_branch_name() {
        let workspaces = vec![
            make_workspace("project-abc12345", Some("main")),
            make_workspace("project-xyz67890", Some("feature/foo")),
        ];
        let result = find_by_id(&workspaces, "feature/foo").unwrap();
        assert_eq!(result, PathBuf::from("/silo/project-xyz67890"));
    }

    #[test]
    fn find_by_id_prefers_directory_name_over_branch() {
        // workspace whose dir name equals another workspace's branch name
        let workspaces = vec![
            make_workspace("my-branch", Some("other-branch")),
            make_workspace("project-xyz67890", Some("my-branch")),
        ];
        let result = find_by_id(&workspaces, "my-branch").unwrap();
        assert_eq!(result, PathBuf::from("/silo/my-branch"));
    }
}
