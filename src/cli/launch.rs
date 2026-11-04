//! Logic for the `launch` command.

use clap::Parser;
use crossterm::style::Color;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::infra::agent::{Agent, AgentCommand};
use crate::infra::git::GitOperations;
use crate::infra::git::validate_branch_name;
use crate::infra::system_process::ProcessOperations;
use crate::infra::terminal::Terminal;
use crate::services::agent_launcher::{
    AgentLauncher, LaunchError, LaunchMode, WorkspaceLaunchOptions,
};
use crate::services::agent_list_service::AgentListService;
use crate::services::git_branch_service::{BranchRenameOutcome, GitBranchService};
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_suggestions_service::GitSuggestionsService;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::silo_config::SiloConfig;
use crate::services::workspace_kind::WorkspaceKind;
use crate::services::workspace_lock::WorkspaceLock;
use crate::services::workspace_manager::{
    WorkspaceListingScope, WorkspaceManager, find_workspace_by_branch,
};
use crate::tui::{Prompt, print_info, print_status};

/// Arguments for the `launch` command.
#[derive(Parser, Debug)]
#[cfg_attr(test, derive(Default))]
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
        active_paths: &HashSet<PathBuf>,
    ) -> Result<std::path::PathBuf, LaunchError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.create(branch, reuse, active_paths),
            WorkspaceBackend::Clone(w) => w.create(branch, reuse, active_paths),
        }
    }
}

impl<G: GitOperations> WorkspaceManager for WorkspaceBackend<G> {
    fn cleanup(
        &self,
        excluded_paths: &std::collections::HashSet<std::path::PathBuf>,
        all: bool,
        force: bool,
    ) -> Result<
        crate::services::workspace_manager::CleanupResult,
        crate::services::workspace_manager::CleanupError,
    > {
        match self {
            WorkspaceBackend::Worktree(w) => w.cleanup(excluded_paths, all, force),
            WorkspaceBackend::Clone(w) => w.cleanup(excluded_paths, all, force),
        }
    }

    fn get_all(
        &self,
        scope: WorkspaceListingScope,
    ) -> Result<Vec<crate::infra::git::GitWorkspaceInfo>, crate::infra::git_error::GitError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.get_all(scope),
            WorkspaceBackend::Clone(w) => w.get_all(scope),
        }
    }
}

/// Handler for the `launch` command.
pub struct LaunchCommand<G: GitOperations, T: Terminal, P: ProcessOperations, U: Prompt> {
    /// Git operations used for workspace creation.
    git: G,
    /// Terminal implementation for windowed launch modes.
    terminal: Option<T>,
    /// Launch strategy for the agent process.
    launch_mode: LaunchMode,
    /// Process inspection used to detect workspaces with running agents.
    process: P,
    /// Interactive prompt for confirmations and commit messages.
    prompt: U,
}

impl<G: GitOperations, T: Terminal, P: ProcessOperations, U: Prompt> LaunchCommand<G, T, P, U> {
    /// Creates a new `LaunchCommand`.
    pub fn new(
        git: G,
        terminal: Option<T>,
        launch_mode: LaunchMode,
        process: P,
        prompt: U,
    ) -> Self {
        Self {
            git,
            terminal,
            launch_mode,
            process,
            prompt,
        }
    }
}

/// What to do in a workspace once an agent launched with [`LaunchMode::ExecReplace`] exits.
struct ExitWorkOptions {
    /// Whether to offer the rename/commit/push flow.
    exit_work_enabled: bool,
    /// Whether the workspace was requested as temporary (`--tmp`).
    is_tmp: bool,
    /// Whether the workspace is the original repository rather than a managed one.
    selected_original: bool,
    /// The kind of workspace the agent ran in.
    kind: WorkspaceKind,
}

impl<G: GitOperations + Clone, T: Terminal, P: ProcessOperations, U: Prompt>
    LaunchCommand<G, T, P, U>
{
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

        let agent = resolve_agent(args.agent.clone());
        let kind = resolve_workspace_type(args.clone, args.worktree);

        print_status(
            "→",
            agent_display_color(&agent),
            &format!("Launching {} in {}…", agent, kind),
        )?;

        let is_exec_replace = self.launch_mode == LaunchMode::ExecReplace;
        let workspace = match kind {
            WorkspaceKind::Clone => {
                WorkspaceBackend::Clone(GitCheckoutWorkspace::new(self.git.clone()))
            }
            WorkspaceKind::Worktree => {
                WorkspaceBackend::Worktree(GitWorktreeWorkspace::new(self.git.clone()))
            }
        };

        let existing_branch_workspace = args.branch.as_deref().and_then(|branch| {
            find_workspace_by_branch(
                &workspace,
                branch,
                WorkspaceListingScope::ManagedAndOriginal,
            )
        });
        let active_paths: HashSet<PathBuf> = if args.reuse || existing_branch_workspace.is_some() {
            AgentListService::new(
                GlobalWorkspaceManager::with_git(self.git.clone()),
                self.process,
            )
            .get_active_paths()?
            .into_iter()
            .collect()
        } else {
            HashSet::new()
        };

        let mut selected_original = false;
        let override_path: Option<PathBuf> = if let (Some(branch_name), Some(existing_path)) =
            (&args.branch, existing_branch_workspace)
        {
            selected_original = workspace_is_original(
                &existing_path,
                &workspace.get_all(WorkspaceListingScope::ManagedOnly)?,
            );
            if active_paths.contains(&existing_path) {
                return Err(format!(
                    "An agent is already running in workspace '{}' at {}",
                    branch_name,
                    existing_path.display()
                )
                .into());
            }
            if WorkspaceLock::is_locked(&existing_path) {
                return Err(format!(
                    "A launch is already in progress for workspace '{}' at {}",
                    branch_name,
                    existing_path.display()
                )
                .into());
            }
            let confirmed = self.prompt.confirm(
                &format!(
                    "Workspace for '{}' already exists at {}. Launch agent there?",
                    branch_name,
                    existing_path.display()
                ),
                true,
            )?;
            if confirmed {
                Some(existing_path)
            } else {
                print_info("Aborted.")?;
                return Ok(());
            }
        } else {
            None
        };

        let launch_result = AgentLauncher::new(
            workspace,
            self.terminal,
            self.launch_mode,
            agent.clone(),
            WorkspaceLaunchOptions {
                branch: args.branch,
                reuse: args.reuse,
                active_paths,
                override_path,
            },
        )
        .launch();

        match launch_result {
            Ok(workspace_path) => {
                print_status("✓", Color::Green, "Agent exited.")?;
                print_info(&format!(
                    "To resume, cd to the workspace:\n  cd {}",
                    workspace_path.display()
                ))?;
                if is_exec_replace {
                    let options = ExitWorkOptions {
                        exit_work_enabled: SiloConfig::load_settings()
                            .ok()
                            .and_then(|s| s.exit_work)
                            .unwrap_or(true),
                        is_tmp: args.tmp,
                        selected_original,
                        kind,
                    };
                    handle_agent_exit(&workspace_path, &agent, &options, &self.git, &self.prompt)?;
                }
                Ok(())
            }
            Err(LaunchError::AgentExitError(status)) => {
                print_status(
                    "✗",
                    Color::Red,
                    &format!("Agent failed with exit status: {}", status),
                )?;
                Err(LaunchError::AgentExitError(status).into())
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Runs the post-exit flow (rename, commit, push, tmp cleanup) for an exec-replace launch.
///
/// Failures of the individual steps are reported as warnings rather than errors.
///
/// # Errors
///
/// Returns an error only if writing a status line fails.
fn handle_agent_exit<G: GitOperations, A: AgentCommand, U: Prompt>(
    workspace_path: &Path,
    agent: &A,
    options: &ExitWorkOptions,
    git: &G,
    prompt: &U,
) -> Result<(), Box<dyn std::error::Error>> {
    if options.exit_work_enabled
        && let Err(e) =
            check_and_handle_exit_work(workspace_path, agent, options.is_tmp, git, prompt)
    {
        print_status(
            "⚠",
            Color::Yellow,
            &format!("Exit work check failed: {}", e),
        )?;
    }
    if should_cleanup_tmp(options.is_tmp, options.selected_original)
        && let Err(e) = cleanup_tmp_workspace(workspace_path, options.kind.clone(), git)
    {
        print_status(
            "⚠",
            Color::Yellow,
            &format!("Tmp workspace cleanup failed: {}", e),
        )?;
    }
    Ok(())
}

/// Returns whether `path` is outside the managed-only workspace set.
fn workspace_is_original(path: &Path, managed: &[crate::infra::git::GitWorkspaceInfo]) -> bool {
    let normalized = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    !managed.iter().any(|workspace| {
        workspace
            .path
            .canonicalize()
            .unwrap_or_else(|_| workspace.path.clone())
            == normalized
    })
}

/// Returns whether temporary-workspace cleanup is safe to run.
fn should_cleanup_tmp(is_tmp: bool, selected_original: bool) -> bool {
    is_tmp && !selected_original
}

/// Returns a display colour for the given agent type.
fn agent_display_color(agent: &Agent) -> Color {
    match agent {
        Agent::ClaudeCode => Color::Cyan,
        Agent::Codex => Color::Yellow,
        Agent::Gemini => Color::Blue,
        Agent::OpenCode => Color::Magenta,
        Agent::Droid => Color::Green,
    }
}

/// Checks for uncommitted or unpushed work after an agent exits and interactively
/// offers to commit, rename the branch, and/or push.
///
/// # Errors
///
/// Returns an error if a TUI interaction fails.
fn check_and_handle_exit_work<G: GitOperations, A: AgentCommand, U: Prompt>(
    workspace_path: &Path,
    agent: &A,
    is_tmp: bool,
    git: &G,
    prompt: &U,
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_tmp {
        handle_branch_rename(workspace_path, agent, git);
    }
    let just_committed = handle_commit_flow(workspace_path, agent, git, prompt)?;
    handle_push_confirmation(workspace_path, just_committed, git, prompt)?;
    Ok(())
}

/// Deletes the workspace at `path` if it has no uncommitted changes and no unpushed commits.
///
/// For worktrees uses `git worktree remove --force`; for clones uses `fs::remove_dir_all`.
/// If the upstream is not configured, treats the branch as clean (nothing to push).
fn cleanup_tmp_workspace<G: GitOperations>(
    path: &Path,
    kind: WorkspaceKind,
    git: &G,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = git.get_status_porcelain(path)?;
    if !status.trim().is_empty() {
        print_status(
            "–",
            Color::DarkGrey,
            &format!(
                "Workspace not deleted: uncommitted changes remain in {}.",
                path.display()
            ),
        )?;
        return Ok(());
    }

    let unpushed = git.count_commits_ahead(path, "@{u}").unwrap_or(0);
    if unpushed > 0 {
        print_status(
            "–",
            Color::DarkGrey,
            &format!(
                "Workspace not deleted: {} unpushed commit(s) remain in {}.",
                unpushed,
                path.display()
            ),
        )?;
        return Ok(());
    }

    let result: Result<(), Box<dyn std::error::Error>> = match kind {
        WorkspaceKind::Worktree => git.remove_worktree(path).map_err(Into::into),
        WorkspaceKind::Clone => std::fs::remove_dir_all(path).map_err(Into::into),
    };

    match result {
        Ok(()) => print_status(
            "✓",
            Color::Green,
            &format!("Temporary workspace deleted: {}", path.display()),
        )?,
        Err(e) => print_status(
            "✗",
            Color::Red,
            &format!("Failed to delete workspace {}: {}", path.display(), e),
        )?,
    }
    Ok(())
}

/// Attempts to rename the auto-generated branch to a descriptive name and reports the outcome.
fn handle_branch_rename<G: GitOperations, A: AgentCommand>(
    workspace_path: &Path,
    agent: &A,
    git: &G,
) {
    let _ = print_status("→", Color::DarkGrey, "Renaming branch…");
    match GitBranchService::new(agent).try_rename(workspace_path, git) {
        BranchRenameOutcome::Skipped => {}
        BranchRenameOutcome::Renamed(name) => {
            let _ = print_status("✓", Color::Green, &format!("Branch renamed to '{}'.", name));
        }
        BranchRenameOutcome::RenameFailed { suggested, error } => {
            let _ = print_status(
                "✗",
                Color::Red,
                &format!("Failed to rename branch to '{}': {}", suggested, error),
            );
        }
        BranchRenameOutcome::SuggestionFailed(e) => {
            let _ = print_status(
                "⚠",
                Color::Yellow,
                &format!("Could not get branch name suggestion: {}", e),
            );
        }
    }
}

/// Interactively prompts the user to commit any uncommitted changes.
///
/// Returns `true` if a commit was successfully created, `false` otherwise.
///
/// # Errors
///
/// Returns an error if a TUI interaction or git status check fails.
fn handle_commit_flow<G: GitOperations, A: AgentCommand, U: Prompt>(
    workspace_path: &Path,
    agent: &A,
    git: &G,
    prompt: &U,
) -> Result<bool, Box<dyn std::error::Error>> {
    let status = git.get_status_porcelain(workspace_path)?;
    if status.trim().is_empty() {
        return Ok(false);
    }

    print_status("–", Color::Yellow, "Uncommitted changes detected:")?;
    for line in status.lines() {
        print_info(&format!("  {}", line))?;
    }

    let should_commit = prompt.confirm("Commit these changes?", true)?;
    if !should_commit {
        return Ok(false);
    }

    print_status(
        "→",
        Color::DarkGrey,
        "Generating commit message suggestion…",
    )?;
    let suggestion = git
        .get_changes_summary(workspace_path)
        .ok()
        .and_then(|changes| {
            GitSuggestionsService::new(agent)
                .suggest_commit_message(&changes)
                .ok()
                .flatten()
        });

    let message = prompt.input("Commit message", suggestion.as_deref())?;
    let Some(message) = message else {
        return Ok(false);
    };

    match git.commit_all(workspace_path, &message) {
        Ok(()) => print_status("✓", Color::Green, "Changes committed.")?,
        Err(e) => {
            print_status("✗", Color::Red, &format!("Failed to commit: {}", e))?;
            print_info("Skipping push step due to failed commit.")?;
            return Ok(false);
        }
    }

    // Verify the commit actually landed before proceeding.
    let post_status = git.get_status_porcelain(workspace_path)?;
    if !post_status.trim().is_empty() {
        print_info("Working tree still has changes after commit; skipping push step.")?;
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
/// Returns an error if a TUI interaction fails.
fn handle_push_confirmation<G: GitOperations, U: Prompt>(
    workspace_path: &Path,
    just_committed: bool,
    git: &G,
    prompt: &U,
) -> Result<(), Box<dyn std::error::Error>> {
    let unpushed = git
        .count_commits_ahead(workspace_path, "@{u}")
        .unwrap_or(if just_committed { 1 } else { 0 });
    let unpushed = unpushed.max(if just_committed { 1 } else { 0 });

    if unpushed == 0 {
        return Ok(());
    }

    print_status(
        "–",
        Color::Yellow,
        &format!("You have {} unpushed commit(s).", unpushed),
    )?;

    let should_push = prompt.confirm("Push changes?", true)?;
    if should_push {
        match git.push(workspace_path) {
            Ok(()) => print_status("✓", Color::Green, "Changes pushed.")?,
            Err(e) => print_status("✗", Color::Red, &format!("Push failed: {}", e))?,
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
            let _ = print_status(
                "⚠",
                Color::Yellow,
                &format!("Failed to load settings.json: {}", err),
            );
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
mod tests {
    use super::*;

    #[test]
    fn tmp_cleanup_is_suppressed_for_original_repository() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = !should_cleanup_tmp(true, true);
        let actual_2 = should_cleanup_tmp(true, false);
        let actual_3 = !should_cleanup_tmp(false, false);
        // Then
        assert!(actual_1);
        assert!(actual_2);
        assert!(actual_3);
    }

    #[test]
    fn original_repository_is_not_considered_managed_for_reuse() {
        // Given
        let original = PathBuf::from("/source/project");
        let managed = vec![crate::infra::git::GitWorkspaceInfo {
            path: PathBuf::from("/silo/project-12345678"),
            ..Default::default()
        }];
        // When
        let actual_1 = workspace_is_original(&original, &managed);
        let actual_2 = !workspace_is_original(&managed[0].path, &managed);
        // Then
        assert!(actual_1);
        assert!(actual_2);
    }

    #[test]
    fn test_shell_quote_simple_string() {
        // Given
        let input = "hello";

        // When
        let result = shell_quote(input);

        // Then
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_shell_quote_empty_string() {
        // Given
        let input = "";

        // When
        let result = shell_quote(input);

        // Then
        assert_eq!(result, "''");
    }

    #[test]
    fn test_shell_quote_string_with_spaces() {
        // Given
        let input = "hello world";

        // When
        let result = shell_quote(input);

        // Then
        assert_eq!(result, "'hello world'");
    }

    #[test]
    fn test_shell_quote_string_with_single_quote() {
        // Given
        let input = "it's";

        // When
        let result = shell_quote(input);

        // Then — POSIX shell escaping replaces ' with '\''
        assert_eq!(result, "'it'\\''s'");
    }

    #[test]
    fn test_shell_quote_multiple_single_quotes() {
        // Given
        let input = "a'b'c";

        // When
        let result = shell_quote(input);

        // Then
        assert_eq!(result, "'a'\\''b'\\''c'");
    }

    #[test]
    fn test_build_silo_launch_command_minimal_args() {
        // Given
        let args = LaunchArgs::default();

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("launch"));
        assert!(!cmd.contains("--agent"));
        assert!(!cmd.contains("--branch"));
        assert!(!cmd.contains("--clone"));
        assert!(!cmd.contains("--worktree"));
        assert!(!cmd.contains("--reuse"));
    }

    #[test]
    fn test_build_silo_launch_command_with_clone_flag() {
        // Given
        let args = LaunchArgs {
            clone: true,
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("--clone"));
        assert!(!cmd.contains("--worktree"));
    }

    #[test]
    fn test_build_silo_launch_command_with_worktree_flag() {
        // Given
        let args = LaunchArgs {
            worktree: true,
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("--worktree"));
        assert!(!cmd.contains("--clone"));
    }

    #[test]
    fn test_build_silo_launch_command_with_branch() {
        // Given
        let args = LaunchArgs {
            branch: Some("my-feature".to_string()),
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("--branch"));
        assert!(cmd.contains("'my-feature'"));
    }

    #[test]
    fn test_build_silo_launch_command_with_agent() {
        // Given
        let args = LaunchArgs {
            agent: Some(Agent::Codex),
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("--agent"));
        assert!(cmd.contains("'codex'"));
    }

    #[test]
    fn test_build_silo_launch_command_with_reuse() {
        // Given
        let args = LaunchArgs {
            reuse: true,
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("--reuse"));
    }

    #[test]
    fn test_build_silo_launch_command_split_pane_flag_not_included() {
        // Given — split_pane is consumed before this function is called (handled in run())
        let args = LaunchArgs {
            split_pane: true,
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(!cmd.contains("--split-pane"));
    }

    #[test]
    fn test_resolve_workspace_type_clone_flag_wins() {
        // Given
        let (clone, worktree) = (true, false);

        // When
        let kind = resolve_workspace_type(clone, worktree);

        // Then
        assert_eq!(kind, WorkspaceKind::Clone);
    }

    #[test]
    fn test_resolve_workspace_type_worktree_flag_wins() {
        // Given
        let (clone, worktree) = (false, true);

        // When
        let kind = resolve_workspace_type(clone, worktree);

        // Then
        assert_eq!(kind, WorkspaceKind::Worktree);
    }

    #[test]
    fn test_resolve_agent_returns_explicit_agent() {
        // Given
        let explicit = [
            Agent::Codex,
            Agent::Gemini,
            Agent::OpenCode,
            Agent::ClaudeCode,
        ];

        // When
        let resolved: Vec<Agent> = explicit
            .iter()
            .map(|agent| resolve_agent(Some(agent.clone())))
            .collect();

        // Then — any explicit agent arg is passed through unchanged.
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn test_build_silo_launch_command_with_tmp() {
        // Given
        let args = LaunchArgs {
            tmp: true,
            ..Default::default()
        };

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(cmd.contains("--tmp"));
    }

    #[test]
    fn test_build_silo_launch_command_without_tmp_not_included() {
        // Given
        let args = LaunchArgs::default();

        // When
        let cmd = build_silo_launch_command(&args);

        // Then
        assert!(!cmd.contains("--tmp"));
    }

    #[test]
    fn test_cleanup_tmp_workspace_skips_when_uncommitted_changes() {
        // Given
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok("M  src/main.rs".to_string()));
        mock.expect_remove_worktree().never();

        // When
        let result = cleanup_tmp_workspace(path, WorkspaceKind::Worktree, &mock);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_skips_when_unpushed_commits() {
        // Given
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| Ok(3));
        mock.expect_remove_worktree().never();

        // When
        let result = cleanup_tmp_workspace(path, WorkspaceKind::Worktree, &mock);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_deletes_clean_worktree() {
        // Given
        let mut mock = crate::infra::git::MockGitOperations::new();
        let path = std::path::Path::new("/fake/path");
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| Ok(0));
        mock.expect_remove_worktree().once().returning(|_| Ok(()));

        // When
        let result = cleanup_tmp_workspace(path, WorkspaceKind::Worktree, &mock);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_treats_no_upstream_as_clean() {
        // Given — count_commits_ahead returns Err (no upstream configured) → treated as 0.
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

        // When
        let result = cleanup_tmp_workspace(path, WorkspaceKind::Worktree, &mock);

        // Then
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_tmp_workspace_deletes_clean_clone() {
        // Given — use a real temp dir since remove_dir_all is not behind a trait.
        let tmp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let path = tmp_dir.path();
        let mut mock = crate::infra::git::MockGitOperations::new();
        mock.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        mock.expect_count_commits_ahead().returning(|_, _| Ok(0));

        // When
        let result = cleanup_tmp_workspace(path, WorkspaceKind::Clone, &mock);

        // Then
        assert!(result.is_ok());
        assert!(!path.exists(), "temp dir should have been removed");
        // Prevent TempDir from trying to clean up an already-deleted directory.
        let _ = tmp_dir.keep();
    }
}

#[cfg(test)]
mod command_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::infra::agent::{MockAgentCommand, PromptError};
    use crate::infra::git::test_support::TestRepo;
    use crate::infra::git::{Git, MockGitOperations};
    use crate::infra::git_error::GitError;
    use crate::infra::system_process::MockProcessOperations;
    use crate::infra::terminal::TerminalError;
    use crate::services::workspace_manager::WorkspaceFactory;
    use crate::tui::prompt::MockPrompt;

    /// Calls recorded by [`FakeTerminal`].
    #[derive(Debug, Default)]
    struct TerminalCalls {
        /// Paths passed to `open_tab`.
        tabs: Vec<PathBuf>,
        /// Commands passed to `split_pane`.
        splits: Vec<String>,
    }

    /// A terminal that records calls and always succeeds.
    #[derive(Debug, Default, Clone)]
    struct FakeTerminal {
        /// Shared record of calls, inspectable after the command consumed the terminal.
        calls: Rc<RefCell<TerminalCalls>>,
    }

    impl Terminal for FakeTerminal {
        fn open_tab(&self, path: &Path, _agent: &Agent) -> Result<(), TerminalError> {
            self.calls.borrow_mut().tabs.push(path.to_path_buf());
            Ok(())
        }

        fn split_pane(&self, _dir: &Path, command: &str) -> Result<(), TerminalError> {
            self.calls.borrow_mut().splits.push(command.to_string());
            Ok(())
        }
    }

    /// A process inspector that finds `pids` running in `cwd`.
    fn processes(pids: Vec<u32>, cwd: PathBuf) -> MockProcessOperations {
        let mut process = MockProcessOperations::new();
        process
            .expect_find_processes_by_names()
            .returning(move |_| Ok(pids.iter().map(|p| (*p, "codex".to_string())).collect()));
        process
            .expect_get_process_cwd()
            .returning(move |_| Ok(cwd.clone()));
        process
    }

    /// Args selecting the existing `main` branch of a test repository.
    fn main_branch_args() -> LaunchArgs {
        LaunchArgs {
            branch: Some("main".to_string()),
            agent: Some(Agent::Codex),
            worktree: true,
            ..Default::default()
        }
    }

    /// A prompt that answers the "launch in existing workspace?" question with `answer`.
    fn confirming(answer: bool) -> MockPrompt {
        let mut prompt = MockPrompt::new();
        prompt
            .expect_confirm()
            .withf(|message, default_yes| message.contains("already exists") && *default_yes)
            .times(1)
            .returning(move |_, _| Ok(answer));
        prompt
    }

    #[test]
    fn run_rejects_invalid_branch_names() {
        // Given
        let command = LaunchCommand::new(
            Git::new(),
            None::<FakeTerminal>,
            LaunchMode::ExecReplace,
            MockProcessOperations::new(),
            MockPrompt::new(),
        );
        // When
        let err = command
            .run(LaunchArgs {
                branch: Some("bad name".to_string()),
                ..Default::default()
            })
            .unwrap_err();
        // Then
        assert!(err.to_string().starts_with("invalid --branch"));
    }

    #[test]
    fn run_split_pane_requires_terminal() {
        // Given
        let command = LaunchCommand::new(
            Git::new(),
            None::<FakeTerminal>,
            LaunchMode::SplitPane,
            MockProcessOperations::new(),
            MockPrompt::new(),
        );
        // When
        let err = command.run(LaunchArgs::default()).unwrap_err();
        // Then
        assert_eq!(err.to_string(), "no terminal provided for split pane");
    }

    #[test]
    fn run_split_pane_relaunches_in_new_pane() {
        // Given
        let terminal = FakeTerminal::default();
        let command = LaunchCommand::new(
            Git::new(),
            Some(terminal.clone()),
            LaunchMode::SplitPane,
            MockProcessOperations::new(),
            MockPrompt::new(),
        );
        command
            .run(LaunchArgs {
                branch: Some("feature".to_string()),
                split_pane: true,
                ..Default::default()
            })
            .unwrap();
        // When
        let calls = terminal.calls.borrow();
        // Then
        assert_eq!(calls.splits.len(), 1);
        assert!(calls.splits[0].contains(" launch --branch 'feature'"));
        assert!(calls.tabs.is_empty());
    }

    #[test]
    fn run_aborts_when_existing_workspace_is_declined() {
        // Given
        let repo = TestRepo::new();
        let terminal = FakeTerminal::default();
        let command = LaunchCommand::new(
            repo.git(),
            Some(terminal.clone()),
            LaunchMode::NewTab,
            processes(vec![], repo.work.clone()),
            confirming(false),
        );
        // When
        command.run(main_branch_args()).unwrap();
        // Then
        assert!(terminal.calls.borrow().tabs.is_empty());
    }

    #[test]
    fn run_opens_tab_in_existing_workspace() {
        // Given
        let repo = TestRepo::new();
        let terminal = FakeTerminal::default();
        let command = LaunchCommand::new(
            repo.git(),
            Some(terminal.clone()),
            LaunchMode::NewTab,
            processes(vec![], repo.work.clone()),
            confirming(true),
        );
        // When
        command.run(main_branch_args()).unwrap();
        // Then
        assert_eq!(terminal.calls.borrow().tabs, vec![repo.work.clone()]);
        assert!(!WorkspaceLock::is_locked(&repo.work), "lock is released");
    }

    #[test]
    fn run_reports_missing_terminal_for_new_tab() {
        // Given
        let repo = TestRepo::new();
        let command = LaunchCommand::new(
            repo.git(),
            None::<FakeTerminal>,
            LaunchMode::NewTab,
            processes(vec![], repo.work.clone()),
            confirming(true),
        );
        // When
        let err = command.run(main_branch_args()).unwrap_err();
        // Then
        assert!(err.to_string().contains("no terminal provided for new tab"));
    }

    #[test]
    fn run_refuses_workspace_with_running_agent() {
        // Given
        let repo = TestRepo::new();
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().never();
        let command = LaunchCommand::new(
            repo.git(),
            Some(FakeTerminal::default()),
            LaunchMode::NewTab,
            processes(vec![4242], repo.work.clone()),
            prompt,
        );
        // When
        let err = command.run(main_branch_args()).unwrap_err();
        // Then
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn run_refuses_locked_workspace() {
        // Given
        let repo = TestRepo::new();
        let _guard = WorkspaceLock::new(&repo.work).try_acquire().unwrap();
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().never();
        let command = LaunchCommand::new(
            repo.git(),
            Some(FakeTerminal::default()),
            LaunchMode::NewTab,
            processes(vec![], repo.work.clone()),
            prompt,
        );
        // When
        let err = command.run(main_branch_args()).unwrap_err();
        // Then
        assert!(err.to_string().contains("already in progress"));
    }

    /// A git mock for a workspace whose status is `status` before and `after` after committing.
    fn dirty_git(status: &'static str, after: &'static str) -> MockGitOperations {
        let mut git = MockGitOperations::new();
        let mut calls = 0;
        git.expect_get_status_porcelain().returning(move |_| {
            calls += 1;
            Ok(if calls == 1 { status } else { after }.to_string())
        });
        git
    }

    /// An agent that replies to every prompt with `reply`.
    fn agent(reply: &'static str) -> MockAgentCommand {
        let mut agent = MockAgentCommand::new();
        agent
            .expect_prompt()
            .returning(move |_| Ok(reply.to_string()));
        agent
    }

    #[test]
    fn commit_flow_skips_clean_workspace() {
        // Given
        let git = dirty_git("", "");
        // When
        let committed =
            handle_commit_flow(Path::new("/ws"), &agent(""), &git, &MockPrompt::new()).unwrap();
        // Then
        assert!(!committed);
    }

    #[test]
    fn commit_flow_respects_declined_commit() {
        // Given
        let git = dirty_git(" M a.rs\n?? b.rs", "");
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().times(1).returning(|_, _| Ok(false));
        prompt.expect_input().never();
        // When
        let committed = handle_commit_flow(Path::new("/ws"), &agent(""), &git, &prompt).unwrap();
        // Then
        assert!(!committed);
    }

    #[test]
    fn commit_flow_commits_with_suggested_message() {
        // Given
        let mut git = dirty_git(" M a.rs", "");
        git.expect_get_changes_summary()
            .returning(|_| Ok("Diff stat: a.rs".to_string()));
        git.expect_commit_all()
            .withf(|_, message| message == "Edited message")
            .times(1)
            .returning(|_, _| Ok(()));
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().returning(|_, _| Ok(true));
        prompt
            .expect_input()
            .withf(|_, initial| *initial == Some("Update a.rs"))
            .times(1)
            .returning(|_, _| Ok(Some("Edited message".to_string())));
        // When
        let committed =
            handle_commit_flow(Path::new("/ws"), &agent("Update a.rs"), &git, &prompt).unwrap();
        // Then
        assert!(committed);
    }

    #[test]
    fn commit_flow_without_suggestion_and_cancelled_message() {
        // Given
        let mut git = dirty_git(" M a.rs", "");
        git.expect_get_changes_summary()
            .returning(|_| Err(GitError::CommandFailed("diff failed".to_string())));
        git.expect_commit_all().never();
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().returning(|_, _| Ok(true));
        prompt
            .expect_input()
            .withf(|_, initial| initial.is_none())
            .returning(|_, _| Ok(None));
        // When
        let committed = handle_commit_flow(Path::new("/ws"), &agent(""), &git, &prompt).unwrap();
        // Then
        assert!(!committed);
    }

    #[test]
    fn commit_flow_reports_commit_failure() {
        // Given
        let mut git = dirty_git(" M a.rs", "");
        git.expect_get_changes_summary()
            .returning(|_| Ok("changes".to_string()));
        git.expect_commit_all()
            .returning(|_, _| Err(GitError::CommitFailed("hook rejected".to_string())));
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().returning(|_, _| Ok(true));
        prompt
            .expect_input()
            .returning(|_, _| Ok(Some("msg".to_string())));
        // When
        let committed = handle_commit_flow(Path::new("/ws"), &agent("msg"), &git, &prompt).unwrap();
        // Then
        assert!(!committed);
    }

    #[test]
    fn commit_flow_detects_changes_left_after_commit() {
        // Given
        let mut git = dirty_git(" M a.rs", "?? still-here.rs");
        git.expect_get_changes_summary()
            .returning(|_| Ok("changes".to_string()));
        git.expect_commit_all().returning(|_, _| Ok(()));
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().returning(|_, _| Ok(true));
        prompt
            .expect_input()
            .returning(|_, _| Ok(Some("msg".to_string())));
        // When
        let committed = handle_commit_flow(Path::new("/ws"), &agent("msg"), &git, &prompt).unwrap();
        // Then
        assert!(!committed);
    }

    #[test]
    fn push_confirmation_skips_when_nothing_to_push() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_count_commits_ahead().returning(|_, _| Ok(0));
        git.expect_push().never();
        // When
        handle_push_confirmation(Path::new("/ws"), false, &git, &MockPrompt::new()).unwrap();
        // Then
        // completing without error is the assertion
    }

    #[test]
    fn push_confirmation_pushes_fresh_commit_without_upstream() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_count_commits_ahead()
            .returning(|_, _| Err(GitError::CommandFailed("no upstream".to_string())));
        git.expect_push().times(1).returning(|_| Ok(()));
        let mut prompt = MockPrompt::new();
        prompt
            .expect_confirm()
            .withf(|message, _| message == "Push changes?")
            .returning(|_, _| Ok(true));
        // When
        handle_push_confirmation(Path::new("/ws"), true, &git, &prompt).unwrap();
        // Then
        // completing without error is the assertion
    }

    #[test]
    fn push_confirmation_reports_push_failure_and_decline() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_count_commits_ahead().returning(|_, _| Ok(2));
        git.expect_push()
            .times(1)
            .returning(|_| Err(GitError::PushFailed("rejected".to_string())));
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().times(1).returning(|_, _| Ok(true));
        handle_push_confirmation(Path::new("/ws"), false, &git, &prompt).unwrap();
        let mut git = MockGitOperations::new();
        git.expect_count_commits_ahead().returning(|_, _| Ok(2));
        git.expect_push().never();
        let mut prompt = MockPrompt::new();
        prompt.expect_confirm().times(1).returning(|_, _| Ok(false));
        // When
        handle_push_confirmation(Path::new("/ws"), false, &git, &prompt).unwrap();
        // Then
        // completing without error is the assertion
    }

    /// A git mock for an auto-generated branch eligible for renaming.
    fn renamable_git() -> MockGitOperations {
        let mut git = MockGitOperations::new();
        git.expect_get_current_branch()
            .returning(|_| Ok(Some("proj-a1b2c3d4".to_string())));
        git.expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        git.expect_count_commits_ahead().returning(|_, _| Ok(1));
        git.expect_get_changes_summary()
            .returning(|_| Ok("Recent commits: x".to_string()));
        git
    }

    #[test]
    fn branch_rename_skips_user_named_branches() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_current_branch()
            .returning(|_| Ok(Some("feature".to_string())));
        git.expect_rename_branch().never();

        // When
        handle_branch_rename(Path::new("/ws"), &agent(""), &git);

        // Then
        // (the mock verifies on drop that no rename was attempted)
    }

    #[test]
    fn branch_rename_renames_auto_generated_branch() {
        // Given
        let mut git = renamable_git();
        git.expect_rename_branch().times(1).returning(|_, _| Ok(()));

        // When
        handle_branch_rename(Path::new("/ws"), &agent("new-name"), &git);

        // Then
        // (the mock verifies on drop that exactly one rename was attempted)
    }

    #[test]
    fn branch_rename_reports_failed_rename() {
        // Given
        let mut git = renamable_git();
        git.expect_rename_branch()
            .times(1)
            .returning(|_, _| Err(GitError::CommandFailed("locked".to_string())));

        // When
        handle_branch_rename(Path::new("/ws"), &agent("new-name"), &git);

        // Then
        // (the failure is reported, not raised; the mock verifies the attempt on drop)
    }

    #[test]
    fn branch_rename_reports_failed_suggestion() {
        // Given
        let mut failing = MockAgentCommand::new();
        failing
            .expect_prompt()
            .returning(|_| Err(PromptError::Failed("offline".to_string())));
        let mut git = renamable_git();
        git.expect_rename_branch().never();

        // When
        handle_branch_rename(Path::new("/ws"), &failing, &git);

        // Then
        // (no rename is attempted; the mock verifies on drop)
    }

    #[test]
    fn exit_work_skips_rename_for_tmp_workspaces() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_current_branch().never();
        git.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        git.expect_count_commits_ahead().returning(|_, _| Ok(0));
        // When
        check_and_handle_exit_work(Path::new("/ws"), &agent(""), true, &git, &MockPrompt::new())
            .unwrap();
        // Then
        // completing without error is the assertion
    }

    #[test]
    fn agent_exit_reports_failed_steps_as_warnings() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_status_porcelain()
            .returning(|_| Err(GitError::CommandFailed("status failed".to_string())));
        let options = ExitWorkOptions {
            exit_work_enabled: true,
            is_tmp: true,
            selected_original: false,
            kind: WorkspaceKind::Worktree,
        };
        // When
        handle_agent_exit(
            Path::new("/ws"),
            &agent(""),
            &options,
            &git,
            &MockPrompt::new(),
        )
        .unwrap();
        // Then
        // completing without error is the assertion
    }

    #[test]
    fn agent_exit_cleans_tmp_workspace_without_exit_work() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        git.expect_count_commits_ahead().returning(|_, _| Ok(0));
        git.expect_remove_worktree().times(1).returning(|_| Ok(()));
        let options = ExitWorkOptions {
            exit_work_enabled: false,
            is_tmp: true,
            selected_original: false,
            kind: WorkspaceKind::Worktree,
        };
        // When
        handle_agent_exit(
            Path::new("/ws"),
            &agent(""),
            &options,
            &git,
            &MockPrompt::new(),
        )
        .unwrap();
        // Then
        // completing without error is the assertion
    }

    #[test]
    fn agent_exit_never_cleans_original_repository() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_status_porcelain().never();
        git.expect_remove_worktree().never();
        let options = ExitWorkOptions {
            exit_work_enabled: false,
            is_tmp: true,
            selected_original: true,
            kind: WorkspaceKind::Worktree,
        };
        // When
        handle_agent_exit(
            Path::new("/ws"),
            &agent(""),
            &options,
            &git,
            &MockPrompt::new(),
        )
        .unwrap();
        // Then
        // completing without error is the assertion
    }

    #[test]
    fn cleanup_tmp_workspace_reports_removal_failure() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_status_porcelain()
            .returning(|_| Ok(String::new()));
        git.expect_count_commits_ahead().returning(|_, _| Ok(0));
        git.expect_remove_worktree()
            .returning(|_| Err(GitError::WorktreeRemovalFailed("busy".to_string())));
        // When
        let actual = cleanup_tmp_workspace(Path::new("/ws"), WorkspaceKind::Worktree, &git).is_ok();
        // Then
        assert!(actual);
    }

    #[test]
    fn agent_display_colors_cover_all_agents() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = agent_display_color(&Agent::ClaudeCode);
        let actual_2 = agent_display_color(&Agent::Codex);
        let actual_3 = agent_display_color(&Agent::Gemini);
        let actual_4 = agent_display_color(&Agent::OpenCode);
        let actual_5 = agent_display_color(&Agent::Droid);
        // Then
        assert_eq!(actual_1, Color::Cyan);
        assert_eq!(actual_2, Color::Yellow);
        assert_eq!(actual_3, Color::Blue);
        assert_eq!(actual_4, Color::Magenta);
        assert_eq!(actual_5, Color::Green);
    }

    #[test]
    fn settings_based_defaults_resolve() {
        // Given
        // (the user's real settings.json, whatever it contains)

        // When
        let kind = resolve_workspace_type(false, false);
        let agent = resolve_agent(None);

        // Then
        assert!(matches!(
            kind,
            WorkspaceKind::Worktree | WorkspaceKind::Clone
        ));
        assert!(Agent::all_names().contains(&agent.to_string().as_str()));
    }

    #[test]
    fn workspace_backend_delegates_to_worktree_manager() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_project_name()
            .returning(|| Ok("silo-backend-test".to_string()));
        git.expect_create_worktree()
            .withf(|_, branch| branch == "feature")
            .times(1)
            .returning(|_, _| Ok(()));
        git.expect_list_worktrees().returning(|| Ok(vec![]));
        git.expect_get_repo_root()
            .returning(|| Ok(PathBuf::from("/repo")));
        git.expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        let backend = WorkspaceBackend::Worktree(GitWorktreeWorkspace::new(git));
        // When
        let path = backend
            .create(Some("feature".to_string()), false, &HashSet::new())
            .unwrap();
        // Then
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("silo-backend-test-")
        );
        assert!(
            backend
                .get_all(WorkspaceListingScope::ManagedOnly)
                .unwrap()
                .is_empty()
        );
        assert!(
            backend
                .cleanup(&HashSet::new(), false, false)
                .unwrap()
                .removed
                .is_empty()
        );
    }

    #[test]
    fn workspace_backend_delegates_to_clone_manager() {
        // Given
        let mut git = MockGitOperations::new();
        git.expect_get_project_name()
            .returning(|| Ok("silo-backend-test-clone".to_string()));
        git.expect_get_repo_root()
            .returning(|| Ok(PathBuf::from("/repo")));
        git.expect_clone_local().times(1).returning(|_, _| Ok(()));
        git.expect_checkout_new_branch()
            .times(1)
            .returning(|_, _| Ok(()));
        git.expect_get_default_remote_branch()
            .returning(|| Ok("origin/main".to_string()));
        // When
        let backend = WorkspaceBackend::Clone(GitCheckoutWorkspace::new(git));
        // Then
        assert!(backend.create(None, false, &HashSet::new()).is_ok());
        assert!(
            backend
                .get_all(WorkspaceListingScope::ManagedOnly)
                .unwrap()
                .is_empty()
        );
        assert!(
            backend
                .cleanup(&HashSet::new(), false, false)
                .unwrap()
                .removed
                .is_empty()
        );
    }
}
