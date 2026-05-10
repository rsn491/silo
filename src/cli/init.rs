//! Logic for the `init` command.

use clap::Parser;
use std::io::{self, IsTerminal};

use crossterm::style::Color;

use crate::infra::agent::Agent;
use crate::services::silo_config::SiloConfig;
use crate::services::workspace_kind::WorkspaceKind;
use crate::tui::{SelectItem, print_status, run_confirm, run_select};

/// Arguments for the `init` command.
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Default agent to use (skips interactive prompt).
    #[arg(long)]
    pub agent: Option<Agent>,

    /// Default workspace type: worktree (default) or checkout.
    #[arg(long, value_name = "TYPE")]
    pub workspace_type: Option<WorkspaceKind>,

    /// Enable commit+push prompts after agent exits (default: true).
    #[arg(long, value_name = "BOOL")]
    pub exit_work: Option<bool>,
}

/// Handler for the `init` command.
pub struct InitCommand;

impl InitCommand {
    /// Creates a new `InitCommand`.
    pub fn new() -> Self {
        Self
    }

    /// Executes the initialization process.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation or configuration saving fails.
    pub fn run(&self, args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
        match SiloConfig::initialize() {
            Ok(path) => {
                print_status("✓", Color::Green, "Silo directory initialized successfully.")?;
                print_status(
                    "→",
                    Color::Cyan,
                    &format!("Future worktrees will be created in: {}", path.display()),
                )?;
                print_status("→", Color::DarkGrey, "Run 'silo launch' to create worktrees.")?;
            }
            Err(e) => {
                print_status("✗", Color::Red, &format!("Error initializing silo directory: {}", e))?;
                std::process::exit(1);
            }
        }

        let is_tty = io::stdin().is_terminal();
        let settings_path = SiloConfig::get_settings_path()?;

        if !is_tty
            && args.agent.is_none()
            && args.workspace_type.is_none()
            && args.exit_work.is_none()
        {
            print_status("–", Color::DarkGrey, "Non-interactive init; skipping settings.json.")?;
            print_status("→", Color::DarkGrey, "Use `silo init --agent <name>` to set the default agent.")?;
            return Ok(());
        }

        // Load existing settings so we only update what the user specifies.
        let mut settings = SiloConfig::load_settings().unwrap_or_default();

        let agent = if let Some(agent) = args.agent {
            Some(agent)
        } else if is_tty {
            prompt_for_agent()?
        } else {
            None
        };

        let workspace_type = if let Some(wt) = args.workspace_type {
            Some(wt)
        } else if is_tty {
            prompt_for_workspace_type()?
        } else {
            None
        };

        let exit_work = if let Some(ew) = args.exit_work {
            Some(ew)
        } else if is_tty {
            prompt_for_exit_work()?
        } else {
            None
        };

        if agent.is_none() && workspace_type.is_none() && exit_work.is_none() {
            print_status("–", Color::DarkGrey, "No settings selected; skipping settings.json.")?;
            return Ok(());
        }

        if settings_path.exists() && is_tty && !confirm_overwrite()? {
            print_status("–", Color::DarkGrey, "Existing settings.json left unchanged.")?;
            return Ok(());
        }

        if let Some(agent) = agent {
            settings.agent = Some(agent);
        }
        if let Some(wt) = workspace_type {
            settings.workspace_type = Some(wt);
        }
        if let Some(ew) = exit_work {
            settings.exit_work = Some(ew);
        }

        SiloConfig::save_settings(&settings)?;
        print_status(
            "✓",
            Color::Green,
            &format!("Saved settings to {}", settings_path.display()),
        )?;

        Ok(())
    }
}

/// Prompts the user to choose a default AI agent.
fn prompt_for_agent() -> Result<Option<Agent>, Box<dyn std::error::Error>> {
    let names = Agent::all_names();
    let mut items: Vec<SelectItem> = names
        .iter()
        .map(|n| SelectItem {
            label: n.to_string(),
            detail: None,
        })
        .collect();
    items.push(SelectItem {
        label: "Skip".to_string(),
        detail: Some("Leave agent setting unchanged".to_string()),
    });

    match run_select("Choose default agent", &items, 0)? {
        Some(i) if i < names.len() => Ok(Agent::try_from_str(names[i]).map(Some).unwrap_or(None)),
        _ => Ok(None),
    }
}

/// Prompts the user to choose a default workspace type.
fn prompt_for_workspace_type() -> Result<Option<WorkspaceKind>, Box<dyn std::error::Error>> {
    let items = vec![
        SelectItem {
            label: "worktree (default)".to_string(),
            detail: Some("Faster; shares git objects with the main repo".to_string()),
        },
        SelectItem {
            label: "clone".to_string(),
            detail: Some("Fully isolated; uses more disk space".to_string()),
        },
        SelectItem {
            label: "Skip".to_string(),
            detail: Some("Leave workspace type setting unchanged".to_string()),
        },
    ];

    match run_select("Choose default workspace type", &items, 0)? {
        Some(0) => Ok(Some(WorkspaceKind::Worktree)),
        Some(1) => Ok(Some(WorkspaceKind::Clone)),
        _ => Ok(None),
    }
}

/// Prompts the user to enable or disable commit+push prompts on agent exit.
fn prompt_for_exit_work() -> Result<Option<bool>, Box<dyn std::error::Error>> {
    let items = vec![
        SelectItem {
            label: "Yes (default)".to_string(),
            detail: Some("Prompt to commit and push when the agent exits".to_string()),
        },
        SelectItem {
            label: "No".to_string(),
            detail: Some("Skip commit/push prompts on agent exit".to_string()),
        },
        SelectItem {
            label: "Skip".to_string(),
            detail: Some("Leave this setting unchanged".to_string()),
        },
    ];

    match run_select("Enable commit+push prompts on agent exit?", &items, 0)? {
        Some(0) => Ok(Some(true)),
        Some(1) => Ok(Some(false)),
        _ => Ok(None),
    }
}

/// Asks the user to confirm overwriting the existing `settings.json` file.
fn confirm_overwrite() -> Result<bool, Box<dyn std::error::Error>> {
    run_confirm("settings.json already exists. Overwrite?", false)
}
