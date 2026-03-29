//! Logic for the `ps` command.

use crate::infra::git::GitOperations;
use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::AgentListService;

/// Handler for the `ps` command.
pub struct PsCommand<G: GitOperations + Clone, P: ProcessOperations> {
    /// Service for enumerating running agents.
    service: AgentListService<G, P>,
}

impl<G: GitOperations + Clone, P: ProcessOperations> PsCommand<G, P> {
    /// Creates a new `PsCommand`.
    pub fn new(service: AgentListService<G, P>) -> Self {
        Self { service }
    }

    /// Executes the `ps` operation to list running agents.
    ///
    /// # Errors
    ///
    /// Returns an error if listing running agents fails.
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let agents = self.service.list_running_agents()?;

        if agents.is_empty() {
            println!("No running agents found in this repository's workspaces.");
        } else {
            println!("{:<8} {:<10} {:<20} WORKSPACE", "PID", "AGENT", "BRANCH");
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
                    agent.path.display()
                );
            }
        }

        Ok(())
    }
}
