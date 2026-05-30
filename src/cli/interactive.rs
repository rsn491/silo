//! Logic for the `interactive` command.

use crate::infra::agent::{Agent, AgentMode};
use crate::infra::git::Git;
use crate::infra::terminal::ITerm2;
use crate::services::agent_launcher::{AgentLauncher, LaunchError, LaunchMode};
use crate::services::git_suggestions_service::GitSuggestionsService;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;
use crate::services::silo_config::SiloConfig;
use crate::tui::AppOutcome;

/// Handler for the `interactive` command.
pub struct InteractiveCommand;

impl InteractiveCommand {
    /// Creates a new [`InteractiveCommand`].
    pub fn new() -> Self {
        Self
    }

    /// Runs the interactive TUI and, if the user presses Enter, launches the agent.
    ///
    /// # Errors
    ///
    /// Returns an error if the TUI fails to initialise or if the agent cannot be launched.
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let default_agent = resolve_default_agent();

        match crate::tui::run(default_agent)? {
            AppOutcome::Quit => Ok(()),
            AppOutcome::Launch {
                agent,
                mode,
                prompt,
            } => {
                let prompt = if prompt.is_empty() {
                    None
                } else {
                    Some(prompt)
                };
                launch_agent(agent, prompt, Some(mode))
            }
        }
    }
}

/// Creates a worktree workspace and exec-replaces the current process with the agent.
fn launch_agent(
    agent: Agent,
    prompt: Option<String>,
    mode: Option<AgentMode>,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Launching {} in worktree...", agent);

    let git = Git;
    let workspace = GitWorktreeWorkspace::new(git.clone());
    let branch = prompt
        .as_deref()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| GitSuggestionsService::new(agent.clone()).suggest_branch_name_from_prompt(p, &git))
        .unwrap_or(Ok(None))
        .unwrap_or_default();

    match AgentLauncher::<_, ITerm2>::new(
        workspace,
        None,
        LaunchMode::ExecReplace,
        agent,
        branch,
        false,
        None,
    )
    .launch(prompt, mode)
    {
        Ok(workspace_path) => {
            eprintln!(
                "\n\nAgent exited. To resume, cd to the workspace:\n  cd {}",
                workspace_path.display()
            );
            Ok(())
        }
        Err(LaunchError::AgentExitError(status)) => {
            eprintln!("\n\nAgent failed with exit status: {}", status);
            Err(LaunchError::AgentExitError(status).into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Loads the default agent from `~/.silo/settings.json`, falling back to `Agent::default()`.
fn resolve_default_agent() -> Agent {
    SiloConfig::load_settings()
        .ok()
        .and_then(|s| s.agent)
        .unwrap_or_default()
}
