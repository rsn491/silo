//! Logic for the `interactive` command.

use crossterm::style::Color;

use crate::infra::agent::{Agent, AgentMode};
use crate::infra::git::Git;
use crate::infra::system_process::SystemProcess;
use crate::infra::terminal::ITerm2;
use crate::services::agent_launcher::{AgentLauncher, LaunchError, LaunchMode};
use crate::services::agent_list_service::AgentListService;
use crate::services::git_suggestions_service::GitSuggestionsService;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;
use crate::services::global_workspace_manager::GlobalWorkspaceManager;
use crate::services::silo_config::SiloConfig;
use crate::tui::{AgentRow, AppOutcome, print_info, print_status};

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
        let service = AgentListService::new(GlobalWorkspaceManager::with_git(Git), SystemProcess);
        let get_agents = move || {
            service
                .list_running_agents()
                .unwrap_or_default()
                .into_iter()
                .map(|a| {
                    let color = match a.agent_type.as_ref() {
                        Some(Agent::ClaudeCode) => ratatui::style::Color::Cyan,
                        Some(Agent::Codex) => ratatui::style::Color::Yellow,
                        Some(Agent::Gemini) => ratatui::style::Color::Blue,
                        Some(Agent::OpenCode) => ratatui::style::Color::Magenta,
                        None => ratatui::style::Color::White,
                    };
                    AgentRow {
                        pid: a.pid,
                        name: a
                            .agent_type
                            .as_ref()
                            .map(|ag| ag.to_string())
                            .unwrap_or_else(|| "(unknown)".to_string()),
                        color,
                        branch: a.branch.unwrap_or_else(|| "(detached)".to_string()),
                        workspace: a.path.display().to_string(),
                    }
                })
                .collect()
        };

        match crate::tui::run(default_agent, get_agents)? {
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
    let agent_color = crate::cli::launch::agent_display_color(&agent);
    print_status(
        "→",
        agent_color,
        &format!("Launching {} in worktree…", agent),
    )?;

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
            print_status("✓", Color::Green, "Agent exited.")?;
            print_info(&format!(
                "To resume, cd to the workspace:\n  cd {}",
                workspace_path.display()
            ))?;
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

/// Loads the default agent from `~/.silo/settings.json`, falling back to `Agent::default()`.
fn resolve_default_agent() -> Agent {
    SiloConfig::load_settings()
        .ok()
        .and_then(|s| s.agent)
        .unwrap_or_default()
}
