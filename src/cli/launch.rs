//! Logic for the `launch` command.

use clap::Parser;
use std::path::Path;
use std::path::PathBuf;

use dialoguer::{Input, Select, theme::ColorfulTheme};

use crate::infra::agent::Agent;
use crate::infra::git::{Git, GitOperations};
use crate::infra::terminal::Terminal;
use crate::services::agent_launcher::{AgentLauncher, LaunchError, LaunchMode};
use crate::services::agent_workspace::WorkspaceFactory;
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
}

/// A wrapper for the different workspace backend implementations.
enum WorkspaceBackend<G: GitOperations> {
    /// Use Git worktrees.
    Worktree(GitWorktreeWorkspace<G>),
    /// Use local Git clones.
    Checkout(GitCheckoutWorkspace<G>),
}

impl<G: GitOperations> WorkspaceFactory for WorkspaceBackend<G> {
    /// Delegates workspace creation to the underlying backend.
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        match self {
            WorkspaceBackend::Worktree(w) => w.create(branch),
            WorkspaceBackend::Checkout(w) => w.create(branch),
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

    /// Executes the launch command.
    ///
    /// # Errors
    ///
    /// Returns an error if workspace creation or agent launching fails.
    pub fn run(self, args: LaunchArgs) -> Result<(), Box<dyn std::error::Error>> {
        let agent = resolve_agent(args.agent);
        let workspace = match resolve_workspace_type(args.checkout, args.worktree) {
            WorkspaceKind::Checkout => {
                WorkspaceBackend::Checkout(GitCheckoutWorkspace::new(self.git))
            }
            WorkspaceKind::Worktree => {
                WorkspaceBackend::Worktree(GitWorktreeWorkspace::new(self.git))
            }
        };
        let workspace_kind = match &workspace {
            WorkspaceBackend::Worktree(_) => "worktree",
            WorkspaceBackend::Checkout(_) => "checkout",
        };
        eprintln!("Launching {:?} in {}...", agent, workspace_kind);

        let is_exec_replace = self.launch_mode == LaunchMode::ExecReplace;
        let agent_for_exit = agent.clone();
        let launch_result = AgentLauncher::new(
            workspace,
            self.terminal,
            self.launch_mode,
            agent,
            args.branch,
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

    // --- Get AI suggestions once (branch name + commit message) ---
    eprintln!("\nGenerating suggestions...");
    let suggestions = match GitSuggestionsService::new(agent.clone()).suggest(workspace_path, &git)
    {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("Warning: could not get suggestions: {}", e);
            None
        }
    };
    let branch_suggestion = suggestions.as_ref().and_then(|s| s.branch_name.as_deref());
    let commit_suggestion = suggestions.as_ref().and_then(|s| s.commit_message.as_deref());

    // --- Step 1: Rename auto-generated branch ---
    match GitBranchService::new().try_rename(workspace_path, &git, branch_suggestion) {
        BranchRenameOutcome::Skipped => {}
        BranchRenameOutcome::Renamed(name) => {
            eprintln!("Branch renamed to '{}'.", name);
        }
        BranchRenameOutcome::RenameFailed { suggested, error } => {
            eprintln!("Failed to rename branch to '{}': {}", suggested, error);
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
            let theme = ColorfulTheme::default();
            let mut commit_input = Input::with_theme(&theme).with_prompt("Commit message");
            if let Some(msg) = commit_suggestion {
                commit_input = commit_input.with_initial_text(msg);
            }
            let message: String = commit_input.interact_text()?;

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
