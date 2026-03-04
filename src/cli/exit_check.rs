//! Interactive check for uncommitted/unpushed work after an agent exits.

use std::path::Path;

use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use crate::infra::git::{Git, GitOperations};

/// Checks for uncommitted or unpushed work after an agent exits and interactively
/// offers to commit and/or push.
///
/// # Errors
///
/// Returns an error if a `dialoguer` interaction fails.
pub fn check_and_handle_exit_work(workspace_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let git = Git::default();

    // --- Step 1: Check for uncommitted changes ---
    let status = git.get_status_porcelain(workspace_path)?;
    if !status.trim().is_empty() {
        eprintln!("\nUncommitted changes detected:");
        for line in status.lines() {
            eprintln!("  {}", line);
        }

        let should_commit = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Commit these changes?")
            .default(true)
            .interact()?;

        if should_commit {
            let message: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Commit message")
                .interact_text()?;

            match git.commit_all(workspace_path, &message) {
                Ok(()) => eprintln!("Changes committed."),
                Err(e) => {
                    eprintln!("Failed to commit: {}", e);
                    return Ok(());
                }
            }
        }
    }

    // --- Step 2: Check for unpushed commits ---
    // Use @{u} to compare against the configured upstream; treat errors as 0 (no upstream set).
    let unpushed = git
        .count_commits_ahead(workspace_path, "@{u}")
        .unwrap_or(0);
    if unpushed > 0 {
        eprintln!("\nYou have {} unpushed commit(s).", unpushed);

        let options = &["Push", "Continue without pushing"];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to do?")
            .items(options)
            .default(0)
            .interact()?;

        if selection == 0 {
            match git.push(workspace_path) {
                Ok(()) => eprintln!("Changes pushed."),
                Err(e) => eprintln!("Push failed: {}", e),
            }
        }
    }

    Ok(())
}
