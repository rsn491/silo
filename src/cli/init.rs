//! Logic for the `init` command.

use clap::Parser;
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use std::io::{self, IsTerminal};

use crate::infra::agent::Agent;
use crate::services::silo_config::SiloConfig;
use crate::services::workspace_kind::WorkspaceKind;

/// Arguments for the `init` command.
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Default agent to use (skips interactive prompt).
    #[arg(long)]
    pub agent: Option<Agent>,

    /// Default workspace type: worktree (default) or checkout.
    #[arg(long, value_name = "TYPE")]
    pub workspace_type: Option<WorkspaceKind>,
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
                println!("Silo directory initialized successfully.");
                println!("Future worktrees will be created in: {}", path.display());
                println!("\nYou can now run 'silo launch' to create worktrees in this directory.");
            }
            Err(e) => {
                eprintln!("Error initializing silo directory: {}", e);
                std::process::exit(1);
            }
        }

        let is_tty = io::stdin().is_terminal();
        let settings_path = SiloConfig::get_settings_path()?;

        if !is_tty && args.agent.is_none() && args.workspace_type.is_none() {
            println!("Non-interactive init detected; skipping settings.json.");
            println!("Use `silo init --agent <name>` to set the default agent.");
            println!("Use `silo init --workspace-type <type>` to set the default workspace type.");
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

        if agent.is_none() && workspace_type.is_none() {
            println!("No settings selected; skipping settings.json.");
            return Ok(());
        }

        if settings_path.exists() && is_tty && !confirm_overwrite()? {
            println!("Existing settings.json left unchanged.");
            return Ok(());
        }

        if let Some(agent) = agent {
            settings.agent = Some(agent);
        }
        if let Some(wt) = workspace_type {
            settings.workspace_type = Some(wt);
        }

        SiloConfig::save_settings(&settings)?;
        println!("Saved settings to {}", settings_path.display());

        Ok(())
    }
}

/// Prompts the user to choose a default AI agent.
fn prompt_for_agent() -> Result<Option<Agent>, Box<dyn std::error::Error>> {
    let names = Agent::all_names();
    let mut items: Vec<&str> = names.clone();
    items.push("Skip");

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose default agent")
        .items(&items)
        .default(0)
        .interact_opt()?;

    match selection {
        Some(i) if i < names.len() => Ok(Agent::try_from_str(names[i]).map(Some).unwrap_or(None)),
        _ => Ok(None),
    }
}

/// Prompts the user to choose a default workspace type.
fn prompt_for_workspace_type() -> Result<Option<WorkspaceKind>, Box<dyn std::error::Error>> {
    let items = &["worktree (default)", "checkout", "Skip"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose default workspace type")
        .items(items)
        .default(0)
        .interact_opt()?;

    match selection {
        Some(0) => Ok(Some(WorkspaceKind::Worktree)),
        Some(1) => Ok(Some(WorkspaceKind::Checkout)),
        _ => Ok(None),
    }
}

/// Asks the user to confirm overwriting the existing `settings.json` file.
fn confirm_overwrite() -> Result<bool, Box<dyn std::error::Error>> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("settings.json already exists. Overwrite?")
        .default(false)
        .interact()?)
}
