//! Logic for the `launch` command.

use clap::Parser;
use std::path::Path;

use dialoguer::{Input, Select, theme::ColorfulTheme};

use crate::infra::agent::Agent;
use crate::infra::git::validate_branch_name;
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
    pub clone: bool,

    /// Use Git worktrees for workspace isolation (overrides settings.json default).
    #[arg(long, group = "workspace")]
    pub worktree: bool,

    /// Reuse an existing inactive workspace (no running agent, work committed and
    /// pushed) instead of always creating a new one. Falls back to creating a new
    /// workspace when no eligible workspace is found.
    #[arg(long)]
    pub reuse: bool,

    /// Delete the workspace after the agent exits if all work is committed and pushed.
    #[arg(long)]
    pub tmp: bool,
}

/// A wrapper for the different workspace backend implementations.
enum WorkspaceBackend<G: GitOperations> {
    /// Use Git worktrees.
    Worktree(GitWorktreeWorkspace<G>),
    /// Use local Git clones.
    Clone(GitCheckoutWorkspace<G>),
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
            WorkspaceBackend::Clone(w) => w.create(branch, reuse),
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
        if let Some(branch) = &args.branch {
            validate_branch_name(branch).map_err(|e| format!("invalid --branch: {}", e))?;
        }

        if self.launch_mode == LaunchMode::SplitPane {
            let terminal = self.terminal.ok_or("no terminal provided for split pane")?;
            let current_dir = std::env::current_dir()?;
            let command = build_silo_launch_command(&args);
            terminal.split_pane(&current_dir, &command)?;
            return Ok(());
        }

        let agent = resolve_agent(args.agent);
        let kind = resolve_workspace_type(args.clone, args.worktree);
        let workspace_kind = kind.clone();
        eprintln!("Launching {:?} in {}...", agent, kind);

        let is_exec_replace = self.launch_mode == LaunchMode::ExecReplace;
        let agent_for_exit = agent.clone();
        let workspace = match kind {
            WorkspaceKind::Clone => WorkspaceBackend::Clone(GitCheckoutWorkspace::new(self.git)),
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
                    if args.tmp
                        && let Err(e) = cleanup_tmp_workspace(&workspace_path, workspace_kind)
                    {
                        eprintln!("Warning: tmp workspace cleanup failed: {}", e);
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
    handle_branch_rename(workspace_path, agent, &git);
    let just_committed = handle_commit_flow(workspace_path, agent, &git)?;
    handle_push_confirmation(workspace_path, just_committed, &git)?;
    Ok(())
}

/// Deletes the workspace at `path` if it has no uncommitted changes and no unpushed commits.
///
/// For worktrees uses `git worktree remove --force`; for clones uses `fs::remove_dir_all`.
/// If the upstream is not configured, treats the branch as clean (nothing to push).
fn cleanup_tmp_workspace(
    path: &Path,
    kind: WorkspaceKind,
) -> Result<(), Box<dyn std::error::Error>> {
    cleanup_tmp_workspace_with_git(path, kind, &Git)
}

/// Inner implementation of [`cleanup_tmp_workspace`] that accepts any [`GitOperations`]
/// implementation, enabling unit testing with mocks.
fn cleanup_tmp_workspace_with_git<G: GitOperations>(
    path: &Path,
    kind: WorkspaceKind,
    git: &G,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = git.get_status_porcelain(path)?;
    if !status.trim().is_empty() {
        eprintln!(
            "\nWorkspace not deleted: uncommitted changes remain in {}.",
            path.display()
        );
        return Ok(());
    }

    let unpushed = git.count_commits_ahead(path, "@{u}").unwrap_or(0);
    if unpushed > 0 {
        eprintln!(
            "\nWorkspace not deleted: {} unpushed commit(s) remain in {}.",
            unpushed,
            path.display()
        );
        return Ok(());
    }

    let result: Result<(), Box<dyn std::error::Error>> = match kind {
        WorkspaceKind::Worktree => git.remove_worktree(path).map_err(Into::into),
        WorkspaceKind::Clone => std::fs::remove_dir_all(path).map_err(Into::into),
    };

    match result {
        Ok(()) => eprintln!("\nTemporary workspace deleted: {}", path.display()),
        Err(e) => eprintln!("\nFailed to delete workspace {}: {}", path.display(), e),
    }
    Ok(())
}

/// Attempts to rename the auto-generated branch to a descriptive name and reports the outcome.
fn handle_branch_rename(workspace_path: &Path, agent: &Agent, git: &Git) {
    eprintln!("\nRenaming branch...");
    match GitBranchService::new(agent.clone()).try_rename(workspace_path, git) {
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
}

/// Interactively prompts the user to commit any uncommitted changes.
///
/// Returns `true` if a commit was successfully created, `false` otherwise.
///
/// # Errors
///
/// Returns an error if a `dialoguer` interaction or git status check fails.
fn handle_commit_flow(
    workspace_path: &Path,
    agent: &Agent,
    git: &Git,
) -> Result<bool, Box<dyn std::error::Error>> {
    let status = git.get_status_porcelain(workspace_path)?;
    if status.trim().is_empty() {
        return Ok(false);
    }

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

    if !should_commit {
        return Ok(false);
    }

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
        Ok(()) => eprintln!("Changes committed."),
        Err(e) => {
            eprintln!("Failed to commit: {}", e);
            eprintln!("Skipping push step due to failed commit.");
            return Ok(false);
        }
    }

    // Verify the commit actually landed before proceeding.
    let post_status = git.get_status_porcelain(workspace_path)?;
    if !post_status.trim().is_empty() {
        eprintln!("Working tree still has changes after commit; skipping push step.");
        return Ok(false);
    }

    Ok(true)
}

/// Interactively prompts the user to push any unpushed commits.
///
/// `just_committed` indicates whether the caller just created a commit, which
/// is used as a fallback when no upstream tracking branch is configured yet.
///
/// # Errors
///
/// Returns an error if a `dialoguer` interaction fails.
fn handle_push_confirmation(
    workspace_path: &Path,
    just_committed: bool,
    git: &Git,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use @{u} to compare against the configured upstream; fall back to 1 if we just committed
    // but no upstream is configured (new branch with no remote tracking branch yet).
    let unpushed = git
        .count_commits_ahead(workspace_path, "@{u}")
        .unwrap_or(if just_committed { 1 } else { 0 });
    let unpushed = unpushed.max(if just_committed { 1 } else { 0 });

    if unpushed == 0 {
        return Ok(());
    }

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

    Ok(())
}

/// Determines the workspace type based on CLI arguments and persistent settings.
fn resolve_workspace_type(clone: bool, worktree: bool) -> WorkspaceKind {
    if clone {
        return WorkspaceKind::Clone;
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
    if args.clone {
        parts.push("--clone".to_string());
    } else if args.worktree {
        parts.push("--worktree".to_string());
    }
    if args.reuse {
        parts.push("--reuse".to_string());
    }
    if args.tmp {
        parts.push("--tmp".to_string());
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
            clone: false,
            worktree: false,
            reuse: false,
            tmp: false,
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
        assert!(!cmd.contains("--clone"));
        assert!(!cmd.contains("--worktree"));
        assert!(!cmd.contains("--reuse"));
    }

    #[test]
    fn test_build_silo_launch_command_with_clone_flag() {
        // Arrange
        let args = LaunchArgs {
            clone: true,
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--clone"));
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
        assert!(!cmd.contains("--clone"));
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
    fn test_resolve_workspace_type_clone_flag_wins() {
        // Act
        let kind = resolve_workspace_type(true, false);

        // Assert
        assert_eq!(kind, WorkspaceKind::Clone);
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

    #[test]
    fn test_build_silo_launch_command_with_tmp() {
        // Arrange
        let args = LaunchArgs {
            tmp: true,
            ..Default::default()
        };

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(cmd.contains("--tmp"));
    }

    #[test]
    fn test_build_silo_launch_command_without_tmp_not_included() {
        // Arrange
        let args = LaunchArgs::default();

        // Act
        let cmd = build_silo_launch_command(&args);

        // Assert
        assert!(!cmd.contains("--tmp"));
    }

    #[test]
    fn test_cleanup_tmp_workspace_skips_when_uncommitted_changes() {
        // Arrange
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok("M  src/main.rs".to_string()));
        mock.expect_remove_worktree().never();

        // Act
        let result = cleanup_tmp_workspace_with_git(path, WorkspaceKind::Worktree, &mock);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_skips_when_unpushed_commits() {
        // Arrange
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| Ok(3));
        mock.expect_remove_worktree().never();

        // Act
        let result = cleanup_tmp_workspace_with_git(path, WorkspaceKind::Worktree, &mock);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_deletes_clean_worktree() {
        // Arrange
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| Ok(0));
        mock.expect_remove_worktree().once().returning(|_| Ok(()));

        // Act
        let result = cleanup_tmp_workspace_with_git(path, WorkspaceKind::Worktree, &mock);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_treats_no_upstream_as_clean() {
        // Arrange — count_commits_ahead returns Err (no upstream configured) → treated as 0.
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| {
            Err(crate::infra::git_error::GitError::CommandFailed(
                "no upstream".to_string(),
            ))
        });
        mock.expect_remove_worktree().once().returning(|_| Ok(()));

        // Act
        let result = cleanup_tmp_workspace_with_git(path, WorkspaceKind::Worktree, &mock);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_deletes_clean_clone() {
        // Arrange — use a real temp dir since remove_dir_all is not behind a trait.
        let tmp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let path = tmp_dir.path();
        let mut mock = crate::infra::git::MockGitOperations::new();
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| Ok(0));

        // Act
        let result = cleanup_tmp_workspace_with_git(path, WorkspaceKind::Clone, &mock);

        // Assert
        assert!(result.is_ok());
        assert!(!path.exists(), "temp dir should have been removed");
        // Prevent TempDir from trying to clean up an already-deleted directory.
        let _ = tmp_dir.keep();
    }
}
