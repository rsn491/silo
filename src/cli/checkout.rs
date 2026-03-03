//! Logic for the `checkout` command.

use std::path::PathBuf;

use clap::Parser;
use dialoguer::Select;

use crate::infra::git::{GitOperations, GitWorkspaceInfo};
use crate::services::agent_workspace::WorkspaceManager;
use crate::services::global_workspace::GlobalWorkspaceManager;

/// Arguments for the `checkout` command.
#[derive(Parser, Debug)]
pub struct CheckoutArgs {
    /// The workspace ID (directory name) to switch into.
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

/// Finds a workspace by matching the provided ID against each workspace's directory name.
///
/// # Errors
///
/// Exits with code 1 if no workspace with the given ID is found.
fn find_by_id(
    workspaces: &[GitWorkspaceInfo],
    id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let found = workspaces
        .iter()
        .find(|w| w.path.file_name().and_then(|n| n.to_str()) == Some(id));

    match found {
        Some(w) => Ok(w.path.clone()),
        None => {
            eprintln!("No workspace found with ID '{}'.", id);
            std::process::exit(1);
        }
    }
}

/// Presents an interactive arrow-key list of workspaces and returns the selected path.
///
/// # Errors
///
/// Returns an error if the terminal interaction fails. Exits with code 1 if the user
/// cancels without making a selection.
fn select_interactively(
    workspaces: &[GitWorkspaceInfo],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let items: Vec<String> = workspaces
        .iter()
        .map(|w| {
            let name = w
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let branch = w.branch.as_deref().unwrap_or("(detached)");
            format!("[{:<8}]  {:<32}  ({})", w.kind, name, branch)
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select a workspace")
        .items(&items)
        .interact_opt()?;

    match selection {
        Some(idx) => Ok(workspaces[idx].path.clone()),
        None => {
            eprintln!("No workspace selected.");
            std::process::exit(1);
        }
    }
}

/// Spawns the user's shell with its working directory set to the given workspace path.
///
/// The current process waits for the shell to exit. When the user exits the new shell,
/// control returns to their original session.
///
/// # Errors
///
/// Returns an error if the shell process cannot be spawned.
fn spawn_shell_in(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    eprintln!("Switching to workspace: {}", path.display());
    eprintln!("Type 'exit' to return to your previous session.");

    let status = std::process::Command::new(&shell)
        .current_dir(path)
        .status()?;

    if let Some(code) = status.code() {
        if code != 0 {
            std::process::exit(code);
        }
    }

    Ok(())
}
