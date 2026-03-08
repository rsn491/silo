//! Silo: A CLI tool for managing isolated AI agent workspaces.

mod cli;
mod infra;
mod services;
mod tui;

use clap::{Parser, Subcommand};
use cli::checkout::{CheckoutArgs, CheckoutCommand};
use cli::cleanup::{CleanupArgs, CleanupCommand};
use cli::completions::{CompletionsArgs, CompletionsCommand};
use cli::init::{InitArgs, InitCommand};
use cli::interactive::InteractiveCommand;
use cli::launch::{LaunchArgs, LaunchCommand};
use cli::ps::{PsArgs, PsCommand};
use cli::status::StatusCommand;
use infra::git::Git;
use infra::system_process::SystemProcess;
use infra::terminal;
use services::agent_launcher::LaunchMode;
use services::agent_list_service::AgentListService;
use services::global_workspace_manager::GlobalWorkspaceManager;
use services::silo_config::SiloConfig;

/// The main CLI structure for Silo.
#[derive(Parser)]
#[command(name = "silo")]
#[command(about = "A CLI tool for managing isolated Claude workspaces")]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Open the interactive TUI to configure and launch an agent.
    #[arg(short = 'i', long = "interactive", conflicts_with = "command")]
    pub interactive: bool,
    /// The subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Supported subcommands for Silo.
#[derive(Subcommand)]
pub enum Commands {
    /// Create a new Git worktree and launch an agent in it.
    Launch(LaunchArgs),
    /// List running agents in worktrees of the current repository.
    Ps(PsArgs),
    /// Initialize the `.silo` directory in your home directory.
    Init(InitArgs),
    /// Clean up worktrees where no agents are running.
    Cleanup(CleanupArgs),
    /// Show status of all worktrees inside the workspace root directory.
    Status,
    /// Generate shell completion scripts.
    Completions(CompletionsArgs),
    /// Switch into a workspace by spawning a new shell in its directory.
    Checkout(CheckoutArgs),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    if let Err(e) = SiloConfig::initialize() {
        eprintln!("Failed to initialize silo: {}", e);
        std::process::exit(1);
    }

    let cli = Cli::parse();
    if cli.interactive {
        InteractiveCommand::new().run()?;
        return Ok(());
    }

    match cli.command {
        Some(Commands::Launch(args)) => {
            let tab = args.tab;
            let split_pane = args.split_pane;
            let (launch_mode, terminal) = if tab || split_pane {
                let term = terminal::detect_terminal()?;
                if split_pane {
                    (LaunchMode::SplitPane, Some(term))
                } else {
                    (LaunchMode::NewTab, Some(term))
                }
            } else {
                (LaunchMode::ExecReplace, None)
            };

            LaunchCommand::new(Git, terminal, launch_mode).run(args)?;
        }
        Some(Commands::Ps(args)) => {
            PsCommand::new(AgentListService::new(
                GlobalWorkspaceManager::with_git(Git),
                SystemProcess,
            ))
            .run(args)?;
        }
        Some(Commands::Init(args)) => InitCommand::new().run(args)?,
        Some(Commands::Cleanup(args)) => {
            CleanupCommand::new(
                GlobalWorkspaceManager::with_git(Git),
                AgentListService::new(GlobalWorkspaceManager::with_git(Git), SystemProcess),
            )
            .run(args)?;
        }
        Some(Commands::Status) => {
            StatusCommand::new(GlobalWorkspaceManager::with_git(Git)).run()?;
        }
        Some(Commands::Completions(args)) => CompletionsCommand::new().run(args)?,
        Some(Commands::Checkout(args)) => {
            CheckoutCommand::new(GlobalWorkspaceManager::with_git(Git)).run(args)?;
        }
        None => {}
    }

    Ok(())
}
