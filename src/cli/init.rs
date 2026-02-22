use clap::Parser;
use std::io::{self, IsTerminal, Write};

use crate::infra::agent::Agent;
use crate::services::silo_config::{SiloConfig, SiloSettings};

#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Default agent to use (skips interactive prompt)
    #[arg(long)]
    pub agent: Option<Agent>,
}

pub struct InitCommand;

impl InitCommand {
    pub fn new() -> Self {
        Self
    }

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

        if !is_tty && args.agent.is_none() {
            println!("Non-interactive init detected; skipping settings.json.");
            println!("Use `silo init --agent <name>` to set the default agent.");
            return Ok(());
        }

        let agent = if let Some(agent) = args.agent {
            agent
        } else {
            match prompt_for_agent()? {
                Some(agent) => agent,
                None => {
                    println!("No agent selected; skipping settings.json.");
                    return Ok(());
                }
            }
        };

        if settings_path.exists() && is_tty && !confirm_overwrite()? {
            println!("Existing settings.json left unchanged.");
            return Ok(());
        }

        let settings = SiloSettings {
            agent: Some(agent.to_string()),
        };
        SiloConfig::save_settings(&settings)?;
        println!("Saved default agent to {}", settings_path.display());

        Ok(())
    }
}

fn prompt_for_agent() -> Result<Option<Agent>, io::Error> {
    let options = Agent::all_names().join(", ");
    loop {
        print!("Choose default agent ({}): ", options);
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input)?;
        if bytes == 0 {
            return Ok(None);
        }

        let value = input.trim();
        if value.is_empty() {
            continue;
        }

        if let Some(agent) = Agent::try_from_str(value) {
            return Ok(Some(agent));
        }

        println!("Invalid agent '{}'. Valid options: {}", value, options);
    }
}

fn confirm_overwrite() -> Result<bool, io::Error> {
    loop {
        print!("settings.json already exists. Overwrite? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input)?;
        if bytes == 0 {
            return Ok(false);
        }

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "" | "n" | "no" => return Ok(false),
            _ => println!("Please answer 'y' or 'n'."),
        }
    }
}
