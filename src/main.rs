mod infra;
mod services;

use clap::{Parser, Subcommand};
use infra::agent::Agent;
use infra::git::Git;
use infra::process::SystemProcess;
use infra::terminal;
use services::agent_launcher::{AgentLauncher, LaunchMode};
use services::agent_list::AgentListService;
use services::git_worktree_workspace::GitWorktreeWorkspace;
use services::silo_config::SiloConfig;
use services::worktree_cleanup::WorktreeCleanupService;
use std::path::PathBuf;

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
    Init,
    /// Clean up worktrees where no agents are running
    Cleanup(CleanupArgs),
}

#[derive(Parser, Debug)]
pub struct LaunchArgs {
    /// Base directory for the worktree (default: parent of repo)
    #[arg(long)]
    pub worktree_base: Option<PathBuf>,

    /// Custom branch name (default: worktree name)
    #[arg(long)]
    pub branch: Option<String>,

    /// Launch the agent in a new terminal tab instead of replacing the current process
    #[arg(long, group = "windowing")]
    pub tab: bool,

    /// Launch the agent in a vertical split pane
    #[arg(long, group = "windowing")]
    pub split_pane: bool,

    /// Agent command to launch (default: claude)
    #[arg(long, default_value_t = Agent::default())]
    pub agent: Agent,
}

#[derive(Parser, Debug)]
pub struct CleanupArgs {
    /// Clean ALL worktrees in the repo, not just silo-managed ones in ~/.silo/
    #[arg(long)]
    pub all: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Launch(args) => {
            let agent = args.agent;

            let (launch_mode, terminal): (LaunchMode, Option<_>) = if args.tab || args.split_pane {
                let term = terminal::detect_terminal()?;
                if args.split_pane {
                    (LaunchMode::SplitPane, Some(term))
                } else {
                    (LaunchMode::NewTab, Some(term))
                }
            } else {
                (LaunchMode::ExecReplace, None)
            };

            let workspace = GitWorktreeWorkspace::new(Git, args.worktree_base, args.branch);
            let launcher = AgentLauncher::new(workspace, terminal, launch_mode, agent);
            launcher.launch()?;
        }
        Commands::Ps => {
            let agents = AgentListService::new(Git, SystemProcess).list_running_agents()?;

            if agents.is_empty() {
                println!("No running agents found in this repository's worktrees.");
            } else {
                println!("{:<8} {:<10} {:<20} WORKTREE", "PID", "AGENT", "BRANCH");
                for agent in &agents {
                    let agent_name = agent
                        .agent_type
                        .as_ref()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| "(unknown)".to_string());
                    println!(
                        "{:<8} {:<10} {:<20} {}",
                        agent.pid,
                        agent_name,
                        agent.branch.as_deref().unwrap_or("(detached)"),
                        agent.worktree_path.display()
                    );
                }
            }
        }
        Commands::Init => match SiloConfig::initialize() {
            Ok(path) => {
                println!("Silo directory initialized successfully.");
                println!("Future worktrees will be created in: {}", path.display());
                println!("\nYou can now run 'silo launch' to create worktrees in this directory.");
            }
            Err(e) => {
                eprintln!("Error initializing silo directory: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Cleanup(args) => {
            // Prompt for confirmation unless --yes is provided
            if !args.yes {
                use std::io::{self, Write};
                print!("This will remove all inactive worktrees. Continue? [y/N]: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    println!("Cleanup cancelled.");
                    return Ok(());
                }
                println!();
            }

            let service = WorktreeCleanupService::new(Git, SystemProcess);
            service.cleanup(args.all)?;
        }
    };

    Ok(())
}
