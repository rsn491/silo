//! Logic for the `launch` command.

use clap::Parser;
use std::path::Path;

use dialoguer::{Input, Select, theme::ColorfulTheme};

use crate::infra::agent::Agent;
use crate::infra::git::{Git, GitOperations};
use crate::infra::terminal::Terminal;
use crate::services::agent_launcher::{AgentLauncher, LaunchError, LaunchMode};
use crate::services::git_branch_service::{BranchRenameOutcome, GitBranchService};
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_suggestions_service::GitSuggestionsService;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;
use crate::services::silo_config::SiloConfig;
use crate::services::workspace_kind::WorkspaceKind;

/// Arguments for the `launch` command.
#[derive(Parser, Debug)]
pub struct LaunchArgs {
    /// Custom branch name (default: worktree name).
    #[arg(long)]
    pub branch: Option<String>,

    /// Launch the agent in a new terminal tab instead of replacing the current process.
    #[arg(long, group = "windowing")]
    pub tab: bool,

    /// Launch the agent in a vertical split pane.
    #[arg(long, group = "windowing")]
    pub split_pane: bool,

    /// Agent command to launch (default: settings.json or claude).
    #[arg(long)]
    pub agent: Option<Agent>,

    /// Use Git clone instead of Git worktrees for workspace isolation.
    #[arg(long, group = "workspace")]
    pub checkout: bool,

    /// Use Git worktrees for workspace isolation (overrides settings.json default).
    #[arg(long, group = "workspace")]
    pub worktree: bool,

    /// Reuse an existing inactive workspace (no running agent, work committed and
    /// pushed) instead of always creating a new one. Falls back to creating a new
    /// workspace when no eligible workspace is found.
    #[arg(long)]
    pub reuse: bool,
}

/// A wrapper for the different workspace backend implementations.
enum WorkspaceBackend<G: GitOperations> {
    /// Use Git worktrees.
    Worktree(GitWorktreeWorkspace<G>),
    /// Use local Git clones.
    Checkout(GitCheckoutWorkspace<G>),
}

impl<G: GitOperations> crate::services::workspace_manager::WorkspaceFactory
    for WorkspaceBackend<G>
{
    fn create(
        &self,
        branch: Option<String>,
        reuse: bool,
    ) -> Result<std::path::PathBuf, LaunchError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.create(branch, reuse),
            WorkspaceBackend::Checkout(w) => w.create(branch, reuse),
        }
    }
}

/// Handler for the `launch` command.
pub struct LaunchCommand<G: GitOperations, T: Terminal> {
    /// Git operations used for workspace creation.
    git: G,
    /// Terminal implementation for windowed launch modes.
    terminal: Option<T>,
    /// Launch strategy for the agent process.
    launch_mode: LaunchMode,
}

impl<G: GitOperations, T: Terminal> LaunchCommand<G, T> {
    /// Creates a new `LaunchCommand`.
    pub fn new(git: G, terminal: Option<T>, launch_mode: LaunchMode) -> Self {
        Self {
            git,
            terminal,
            launch_mode,
        }
    }
}

impl<G: GitOperations, T: Terminal> LaunchCommand<G, T> {
    /// Executes the launch command.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace creation or agent launching fails.
    pub fn run(self, args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
        if self.launch_mode == LaunchMode::SplitPane {
            let terminal = self.terminal.ok_or("no terminal provided for split pane")?;
            let current_dir = std::env::current_dir()?;
            let command = build_silo_launch_command(&args);
            terminal.split_pane(&current_dir, &command)?;
            return Ok(());
        }

        let agent = resolve_agent(args.agent);
        let kind = resolve_workspace_type(args.checkout, args.worktree);
        eprintln!("Launching {:?} in {}...", agent, kind);

        let is_exec_replace = self.launch_mode == LaunchMode::ExecReplace;
        let agent_for_exit = agent.clone();
        let workspace = match kind {
            WorkspaceKind::Checkout => {
                WorkspaceBackend::Checkout(GitCheckoutWorkspace::new(self.git))
            }
            WorkspaceKind::Worktree => {
                WorkspaceBackend::Worktree(GitWorktreeWorkspace::new(self.git))
            }
        };
        let launch_result = AgentLauncher::new(
            workspace,
            self.terminal,
            self.launch_mode,
            agent,
            args.branch,
            args.reuse,
        )
        .launch();

        match launch_result {
            Ok(workspace_path) => {
                eprintln!(
                    "\n\nAgent exited. To resume, cd to the workspace:\n  cd {}",
                    workspace_path.display()
                );
                if is_exec_replace {
                    let exit_work_enabled = SiloConfig::load_settings()
                        .ok()
                        .and_then(|s| s.exit_work)
                        .unwrap_or(true);
                    if exit_work_enabled
                        && let Err(e) = check_and_handle_exit_work(&workspace_path, &agent_for_exit)
                    {
                        eprintln!("Warning: exit work check failed: {}", e);
                    }
                }
                Ok(())
            }
            Err(LaunchError::AgentExitError(status)) => {
                eprintln!("\n\nAgent failed with exit status: {}", status);
                Err(LaunchError::AgentExitError(status).into())
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Checks for uncommitted or unpushed work after an agent exits and interactively
/// offers to commit, rename the branch, and/or push.
///
/// # Errors
///
/// Returns an error if a `dialoguer` interaction fails.
fn check_and_handle_exit_work(
    workspace_path: &Path,
    agent: &Agent,
) -> Result<(), Box<dyn std::error::Error>> {
    let git = Git;

    // --- Step 1: Rename auto-generated branch ---
    eprintln!("\nRenaming branch...");
    match GitBranchService::new(agent.clone()).try_rename(workspace_path, &git) {
        BranchRenameOutcome::Skipped => {}
        BranchRenameOutcome::Renamed(name) => {
            eprintln!("Branch renamed to '{}'.", name);
        }
        BranchRenameOutcome::RenameFailed { suggested, error } => {
            eprintln!("Failed to rename branch to '{}': {}", suggested, error);
        }
        BranchRenameOutcome::SuggestionFailed(e) => {
            eprintln!("Warning: could not get branch name suggestion: {}", e);
        }
    }

    // --- Step 2: Check for uncommitted changes ---
    let mut just_committed = false;
    let status = git.get_status_porcelain(workspace_path)?;
    if !status.trim().is_empty() {
        eprintln!("\nUncommitted changes detected:");
        for line in status.lines() {
            eprintln!("  {}", line);
        }

        let should_commit = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Commit these changes?")
            .items(["Yes", "No"])
            .default(0)
            .interact()?
            == 0;

        if should_commit {
            eprintln!("Generating commit message suggestion...");
            let suggestion = git
                .get_changes_summary(workspace_path)
                .ok()
                .and_then(|changes| {
                    GitSuggestionsService::new(agent.clone())
                        .suggest_commit_message(&changes)
                        .ok()
                        .flatten()
                });

            let theme = ColorfulTheme::default();
            let mut input = Input::<String>::with_theme(&theme).with_prompt("Commit message");
            if let Some(ref msg) = suggestion {
                input = input.with_initial_text(msg);
            }
            let message = input.interact_text()?;

            match git.commit_all(workspace_path, &message) {
                Ok(()) => {
                    eprintln!("Changes committed.");
                    just_committed = true;
                }
                Err(e) => {
                    eprintln!("Failed to commit: {}", e);
                    eprintln!("Skipping push step due to failed commit.");
                    return Ok(());
                }
            }

            // Verify the commit actually landed before proceeding.
            let post_status = git.get_status_porcelain(workspace_path)?;
            if !post_status.trim().is_empty() {
                eprintln!("Working tree still has changes after commit; skipping push step.");
                return Ok(());
            }
        }
    }

    // --- Step 3: Check for unpushed commits ---
    // Use @{u} to compare against the configured upstream; fall back to 1 if we just committed
    // but no upstream is configured (new branch with no remote tracking branch yet).
    let unpushed = git
        .count_commits_ahead(workspace_path, "@{u}")
        .unwrap_or(if just_committed { 1 } else { 0 });
    if unpushed > 0 || just_committed {
        let unpushed = unpushed.max(if just_committed { 1 } else { 0 });
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

/// Determines the workspace type based on CLI arguments and persistent settings.
fn resolve_workspace_type(checkout: bool, worktree: bool) -> WorkspaceKind {
    if checkout {
        return WorkspaceKind::Checkout;
    }
    if worktree {
        return WorkspaceKind::Worktree;
    }

    let settings = match SiloConfig::load_settings() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Warning: failed to load settings.json: {}", err);
            return WorkspaceKind::Worktree;
        }
    };

    settings.workspace_type.unwrap_or(WorkspaceKind::Worktree)
}

/// Determines which agent to launch based on CLI arguments and persistent settings.
fn resolve_agent(agent: Option<Agent>) -> Agent {
    agent
        .or_else(|| {
            SiloConfig::load_settings()
                .ok()
                .and_then(|settings| settings.agent)
        })
        .unwrap_or_default()
}

/// Builds a `silo launch` command string from the given args, excluding `--split-pane`.
///
/// Uses the current executable path so the new pane runs the same binary.
fn build_silo_launch_command(args: &LaunchArgs) -> String {
    let silo = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "silo".to_string());

    let mut parts = vec![shell_quote(&silo), "launch".to_string()];

    if let Some(agent) = &args.agent {
        parts.push("--agent".to_string());
        parts.push(shell_quote(agent.command_name()));
    }
    if let Some(branch) = &args.branch {
        parts.push("--branch".to_string());
        parts.push(shell_quote(branch));
    }
    if args.checkout {
        parts.push("--checkout".to_string());
    } else if args.worktree {
        parts.push("--worktree".to_string());
    }
    if args.reuse {
        parts.push("--reuse".to_string());
    }

    parts.join(" ")
}

/// Wraps a string in single quotes with internal single quotes escaped for POSIX shell.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
impl Default for LaunchArgs {
    fn default() -> Self {
        Self {
            branch: None,
            tab: false,
            split_pane: false,
            agent: None,
            checkout: false,
            worktree: false,
            reuse: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_quote_simple_string() {
        // Arrange
        let input = "hello";

        // Act
        let result = shell_quote(input);

        // Assert
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_shell_quote_empty_string() {
        // Arrange
        let input = "";

        // Act
        let result = shell_quote(input);

        // Assert
        assert_eq!(result, "''");
    }

    #[test]
    fn test_shell_quote_string_with_spaces() {
        // Arrange
        let input = "hello world";

        // Act
        let result = shell_quote(input);

        // Assert
        assert_eq!(result, "'hello world'");
    }

    #[test]
    fn test_shell_quote_string_with_single_quote() {
        // Arrange
        let input = "it's";

        // Act
        let result = shell_quote(input);

        // Assert — POSIX shell escaping replaces ' with '\''
        assert_eq!(result, "'it'\\''s'");
    }

    #[test]
    fn test_shell_quote_multiple_single_quotes() {
        // Arrange
        let input = "a'b'c";

        // Act
        let result = shell_quote(input);

        // Assert
        assert_eq!(result, "'a'\\''b'\\''c'");
    }

    #[test]
    fn test_build_silo_launch_command_minimal_args() {
        // Arrange
        let args = LaunchArgs::default();

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("launch"));
        assert!(!cmd.contains("--agent"));
        assert!(!cmd.contains("--branch"));
        assert!(!cmd.contains("--checkout"));
        assert!(!cmd.contains("--worktree"));
        assert!(!cmd.contains("--reuse"));
    }

    #[test]
    fn test_build_silo_launch_command_with_checkout_flag() {
        // Arrange
        let args = LaunchArgs {
            checkout: true,
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--checkout"));
        assert!(!cmd.contains("--worktree"));
    }

    #[test]
    fn test_build_silo_launch_command_with_worktree_flag() {
        // Arrange
        let args = LaunchArgs {
            worktree: true,
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--worktree"));
        assert!(!cmd.contains("--checkout"));
    }

    #[test]
    fn test_build_silo_launch_command_with_branch() {
        // Arrange
        let args = LaunchArgs {
            branch: Some("my-feature".to_string()),
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--branch"));
        assert!(cmd.contains("'my-feature'"));
    }

    #[test]
    fn test_build_silo_launch_command_with_agent() {
        // Arrange
        let args = LaunchArgs {
            agent: Some(Agent::Codex),
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--agent"));
        assert!(cmd.contains("'codex'"));
    }

    #[test]
    fn test_build_silo_launch_command_with_reuse() {
        // Arrange
        let args = LaunchArgs {
            reuse: true,
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--reuse"));
    }

    #[test]
    fn test_build_silo_launch_command_split_pane_flag_not_included() {
        // Arrange — split_pane is consumed before this function is called (handled in run())
        let args = LaunchArgs {
            split_pane: true,
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(!cmd.contains("--split-pane"));
    }

    #[test]
    fn test_resolve_workspace_type_checkout_flag_wins() {
        // Act
        let kind = resolve_workspace_type(true, false);

        // Assert
        assert_eq!(kind, WorkspaceKind::Checkout);
    }

    #[test]
    fn test_resolve_workspace_type_worktree_flag_wins() {
        // Act
        let kind = resolve_workspace_type(false, true);

        // Assert
        assert_eq!(kind, WorkspaceKind::Worktree);
    }

    #[test]
    fn test_resolve_agent_returns_explicit_agent() {
        // Assert — any explicit agent arg is passed through unchanged.
        assert_eq!(resolve_agent(Some(Agent::Codex)), Agent::Codex);
        assert_eq!(resolve_agent(Some(Agent::Gemini)), Agent::Gemini);
        assert_eq!(resolve_agent(Some(Agent::OpenCode)), Agent::OpenCode);
        assert_eq!(resolve_agent(Some(Agent::ClaudeCode)), Agent::ClaudeCode);
    }
}
