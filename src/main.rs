mod cli;
mod infra;
mod services;

use clap::{Parser, Subcommand};
use cli::cleanup::{CleanupArgs, CleanupCommand};
use cli::completions::{CompletionsArgs, CompletionsCommand};
use cli::init::{InitArgs, InitCommand};
use cli::launch::{LaunchArgs, LaunchCommand, WorkspaceBackend};
use cli::ps::PsCommand;
use cli::status::{StatusArgs, StatusCommand};
use infra::git::Git;
use infra::process::SystemProcess;
use infra::terminal;
use services::agent_launcher::LaunchMode;
use services::agent_list::AgentListService;
use services::git_checkout_workspace::GitCheckoutWorkspace;
use services::git_worktree_workspace::GitWorktreeWorkspace;
use services::silo_config::SiloConfig;
use services::workspace_cleanup::WorkspaceCleanupService;

#[derive(Parser)]
#[command(name = "silo")]
#[command(about = "A CLI tool for managing isolated Claude workspaces")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new git worktree and launch an agent in it
    Launch(LaunchArgs),
    /// List running agents in worktrees of the current repository
    Ps,
    /// Initialize the .silo directory in your home directory
    Init(InitArgs),
    /// Clean up worktrees where no agents are running
    Cleanup(CleanupArgs),
    /// Show status of worktrees (uncommitted changes and commits ahead/behind)
    Status(StatusArgs),
    /// Generate shell completion scripts
    Completions(CompletionsArgs),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = SiloConfig::initialize() {
        eprintln!("Failed to initialize silo: {}", e);
        std::process::exit(1);
    }

    let cli = Cli::parse();
    match cli.command {
        Commands::Launch(args) => {
            let tab = args.tab;
            let split_pane = args.split_pane;
            let checkout = args.checkout;

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

            let workspace = if checkout {
                WorkspaceBackend::Checkout(GitCheckoutWorkspace::new(Git))
            } else {
                WorkspaceBackend::Worktree(GitWorktreeWorkspace::new(Git))
            };

            LaunchCommand::new(workspace, terminal, launch_mode).run(args)?;
        }
        Commands::Ps => {
            PsCommand::new(AgentListService::new(Git, SystemProcess)).run()?;
        }
        Commands::Init(args) => InitCommand::new().run(args)?,
        Commands::Cleanup(args) => {
            CleanupCommand::new(WorkspaceCleanupService::new(Git, SystemProcess)).run(args)?;
        }
        Commands::Status(args) => {
            StatusCommand::new(
                GitWorktreeWorkspace::new(Git),
                GitCheckoutWorkspace::new(Git),
            )
            .run(args)?;
        }
        Commands::Completions(args) => CompletionsCommand::new().run(args)?,
    }

    Ok(())
}
