use crate::infra::git::Git;
use crate::infra::process::SystemProcess;
use crate::services::agent_list::AgentListService;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
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

    Ok(())
}
