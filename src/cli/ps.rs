use crate::infra::git::GitOperations;
use crate::infra::process::ProcessOperations;
use crate::services::agent_list::AgentListService;

pub struct PsCommand<G: GitOperations + Clone, P: ProcessOperations> {
    service: AgentListService<G, P>,
}

impl<G: GitOperations + Clone, P: ProcessOperations> PsCommand<G, P> {
    pub fn new(service: AgentListService<G, P>) -> Self {
        Self { service }
    }

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
