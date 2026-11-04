//! Logic for the `checkout` command.

use std::path::{Path, PathBuf};

use clap::Parser;
use thiserror::Error;

use crate::infra::git::GitWorkspaceInfo;
use crate::services::workspace_manager::{WorkspaceListingScope, WorkspaceManager};
use crate::tui::{Prompt, SelectItem, pad_or_trunc};

/// Arguments for the `checkout` command.
#[derive(Parser, Debug)]
pub struct CheckoutArgs {
    /// The workspace ID (directory name) or branch name to switch into.
    ///
    /// If omitted, an interactive selector is displayed.
    pub workspace_id: Option<String>,
}

/// Reasons the checkout command cannot pick a workspace; each exits with code 1.
#[derive(Debug, Error, PartialEq)]
enum CheckoutError {
    /// The repository has no workspaces at all.
    #[error("No workspaces found.")]
    NoWorkspaces,
    /// No workspace matched the requested ID or branch.
    #[error("No workspace found with ID or branch '{0}'.")]
    NotFound(String),
    /// The user cancelled the interactive selector.
    #[error("No workspace selected.")]
    NotSelected,
}

/// Handler for the `checkout` command.
pub struct CheckoutCommand<W: WorkspaceManager, U: Prompt> {
    /// Workspace manager used to list available workspaces.
    workspace_manager: W,
    /// Interactive prompt used when no workspace ID is given.
    prompt: U,
}

impl<W: WorkspaceManager, U: Prompt> CheckoutCommand<W, U> {
    /// Creates a new `CheckoutCommand`.
    pub fn new(workspace_manager: W, prompt: U) -> Self {
        Self {
            workspace_manager,
            prompt,
        }
    }

    /// Executes the checkout command.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace listing, user selection, or shell spawning fails.
    pub fn run(self, args: CheckoutArgs) -> Result<(), Box<dyn std::error::Error>> {
        let workspace_path = match self.resolve_workspace(args.workspace_id.as_deref()) {
            Ok(path) => path,
            Err(e) => match e.downcast::<CheckoutError>() {
                Ok(reason) => {
                    eprintln!("{}", reason);
                    std::process::exit(1);
                }
                Err(e) => return Err(e),
            },
        };

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        if let Some(code) = spawn_shell_in(&workspace_path, &shell)?
            && code != 0
        {
            std::process::exit(code);
        }
        Ok(())
    }

    /// Returns the path of the workspace matching `id`, or the one the user selects.
    ///
    /// # Errors
    ///
    /// Returns a [`CheckoutError`] when no workspace can be chosen, or any listing or
    /// prompt error.
    fn resolve_workspace(&self, id: Option<&str>) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let workspaces = self
            .workspace_manager
            .get_all(WorkspaceListingScope::ManagedAndOriginal)?;

        if workspaces.is_empty() {
            return Err(CheckoutError::NoWorkspaces.into());
        }

        match id {
            Some(id) => Ok(find_by_id(&workspaces, id)?),
            None => {
                let items = select_items(&workspaces);
                match self.prompt.select("Select a workspace", &items, 0)? {
                    Some(idx) => Ok(workspaces[idx].path.clone()),
                    None => Err(CheckoutError::NotSelected.into()),
                }
            }
        }
    }
}

/// Finds a workspace by matching the provided query against each workspace's directory name or
/// branch name, whichever matches first.
///
/// # Errors
///
/// Returns [`CheckoutError::NotFound`] if no workspace with the given ID or branch is found.
fn find_by_id(workspaces: &[GitWorkspaceInfo], id: &str) -> Result<PathBuf, CheckoutError> {
    workspaces
        .iter()
        .find(|w| {
            let name_match = w.path.file_name().and_then(|n| n.to_str()) == Some(id);
            let branch_match = w.branch.as_deref() == Some(id);
            name_match || branch_match
        })
        .map(|w| w.path.clone())
        .ok_or_else(|| CheckoutError::NotFound(id.to_string()))
}

/// Builds the selector rows for the interactive workspace picker.
fn select_items(workspaces: &[GitWorkspaceInfo]) -> Vec<SelectItem> {
    workspaces
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
            SelectItem { label, detail }
        })
        .collect()
}

/// Spawns `shell` with its working directory set to the given workspace path and waits for it.
///
/// Returns the shell's exit code (`None` if it was terminated by a signal).
///
/// # Errors
///
/// Returns an error if the shell process cannot be spawned.
fn spawn_shell_in(path: &Path, shell: &str) -> Result<Option<i32>, Box<dyn std::error::Error>> {
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

    let status = std::process::Command::new(shell)
        .current_dir(path)
        .status()?;

    Ok(status.code())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::infra::git_error::GitError;
    use crate::services::workspace_manager::MockWorkspaceManager;
    use crate::tui::prompt::MockPrompt;

    fn make_workspace(dir: &str, branch: Option<&str>) -> GitWorkspaceInfo {
        GitWorkspaceInfo {
            path: PathBuf::from(format!("/silo/{}", dir)),
            branch: branch.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn find_by_id_matches_directory_name() {
        // Given
        let workspaces = vec![
            make_workspace("project-abc12345", Some("main")),
            make_workspace("project-xyz67890", Some("feature/foo")),
        ];
        // When
        let result = find_by_id(&workspaces, "project-abc12345").unwrap();
        // Then
        assert_eq!(result, PathBuf::from("/silo/project-abc12345"));
    }

    #[test]
    fn find_by_id_matches_branch_name() {
        // Given
        let workspaces = vec![
            make_workspace("project-abc12345", Some("main")),
            make_workspace("project-xyz67890", Some("feature/foo")),
        ];
        // When
        let result = find_by_id(&workspaces, "feature/foo").unwrap();
        // Then
        assert_eq!(result, PathBuf::from("/silo/project-xyz67890"));
    }

    #[test]
    fn find_by_id_prefers_directory_name_over_branch() {
        // Given
        // workspace whose dir name equals another workspace's branch name
        let workspaces = vec![
            make_workspace("my-branch", Some("other-branch")),
            make_workspace("project-xyz67890", Some("my-branch")),
        ];
        // When
        let result = find_by_id(&workspaces, "my-branch").unwrap();
        // Then
        assert_eq!(result, PathBuf::from("/silo/my-branch"));
    }

    #[test]
    fn find_by_id_reports_missing_workspace() {
        // Given
        let workspaces = vec![make_workspace("project-abc12345", Some("main"))];
        // When
        let err = find_by_id(&workspaces, "nope").unwrap_err();
        // Then
        assert_eq!(err, CheckoutError::NotFound("nope".to_string()));
        assert_eq!(
            err.to_string(),
            "No workspace found with ID or branch 'nope'."
        );
    }

    /// A workspace manager returning `workspaces` for every listing.
    fn manager_with(workspaces: Vec<GitWorkspaceInfo>) -> MockWorkspaceManager {
        let mut manager = MockWorkspaceManager::new();
        manager
            .expect_get_all()
            .withf(|scope| *scope == WorkspaceListingScope::ManagedAndOriginal)
            .returning(move |_| Ok(workspaces.clone()));
        manager
    }

    /// Extracts the [`CheckoutError`] from a boxed error.
    fn checkout_error(err: Box<dyn std::error::Error>) -> CheckoutError {
        *err.downcast::<CheckoutError>()
            .expect("expected CheckoutError")
    }

    #[test]
    fn resolve_workspace_fails_without_workspaces() {
        // Given
        let command = CheckoutCommand::new(manager_with(vec![]), MockPrompt::new());
        // When
        let err = command.resolve_workspace(Some("x")).unwrap_err();
        // Then
        assert_eq!(checkout_error(err), CheckoutError::NoWorkspaces);
    }

    #[test]
    fn resolve_workspace_by_id() {
        // Given
        let manager = manager_with(vec![make_workspace("project-abc12345", Some("main"))]);
        let mut prompt = MockPrompt::new();
        prompt.expect_select().never();
        let command = CheckoutCommand::new(manager, prompt);
        // When
        let actual_1 = command.resolve_workspace(Some("main")).unwrap();
        let actual_2 = checkout_error(command.resolve_workspace(Some("other")).unwrap_err());
        // Then
        assert_eq!(actual_1, PathBuf::from("/silo/project-abc12345"));
        assert_eq!(actual_2, CheckoutError::NotFound("other".to_string()));
    }

    #[test]
    fn resolve_workspace_interactively() {
        // Given
        let mut busy = make_workspace("project-xyz67890", None);
        busy.has_uncommitted_changes = true;
        busy.commits_ahead = 2;
        busy.latest_commit = Some("abc123 wip".to_string());
        let manager = manager_with(vec![make_workspace("project-abc12345", Some("main")), busy]);
        let mut prompt = MockPrompt::new();
        prompt
            .expect_select()
            .withf(|_, items, default| {
                *default == 0
                    && items.len() == 2
                    && items[0].detail.is_none()
                    && items[1].label.contains("(detached)")
                    && items[1].label.contains("abc123 wip")
                    && items[1].detail.as_deref() == Some("uncommitted changes · 2 commit(s) ahead")
            })
            .times(1)
            .returning(|_, _, _| Ok(Some(1)));
        let command = CheckoutCommand::new(manager, prompt);
        // When
        let actual = command.resolve_workspace(None).unwrap();
        // Then
        assert_eq!(actual, PathBuf::from("/silo/project-xyz67890"));
    }

    #[test]
    fn resolve_workspace_cancelled_selection() {
        // Given
        let manager = manager_with(vec![make_workspace("project-abc12345", Some("main"))]);
        let mut prompt = MockPrompt::new();
        prompt.expect_select().returning(|_, _, _| Ok(None));
        let command = CheckoutCommand::new(manager, prompt);
        // When
        let err = command.resolve_workspace(None).unwrap_err();
        // Then
        assert_eq!(checkout_error(err), CheckoutError::NotSelected);
    }

    #[test]
    fn resolve_workspace_propagates_listing_errors() {
        // Given
        let mut manager = MockWorkspaceManager::new();
        manager
            .expect_get_all()
            .returning(|_| Err(GitError::NotAGitRepo));
        let command = CheckoutCommand::new(manager, MockPrompt::new());
        // When
        let err = command.resolve_workspace(None).unwrap_err();
        // Then
        assert!(err.downcast::<CheckoutError>().is_err());
    }

    #[test]
    fn spawn_shell_in_reports_exit_code() {
        // Given
        let tmp = tempfile::TempDir::new().unwrap();
        // When
        let actual_1 = spawn_shell_in(tmp.path(), "true").unwrap();
        let actual_2 = spawn_shell_in(tmp.path(), "false").unwrap();
        let actual_3 = spawn_shell_in(tmp.path(), "silo-definitely-missing-shell").is_err();
        // Then
        assert_eq!(actual_1, Some(0));
        assert_eq!(actual_2, Some(1));
        assert!(actual_3);
    }
}
