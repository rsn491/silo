use std::collections::HashSet;
use std::path::PathBuf;

use crate::infra::git::{GitOperations, WorktreeInfo};
use crate::infra::git_error::GitError;
use crate::infra::process::ProcessOperations;
use crate::services::agent_list::AgentListService;
use crate::services::silo_config::SiloConfig;

pub struct WorktreeCleanupService<G: GitOperations + Clone, P: ProcessOperations + Clone> {
    git: G,
    agent_list_service: AgentListService<G, P>,
}

impl<G: GitOperations + Clone, P: ProcessOperations + Clone> WorktreeCleanupService<G, P> {
    pub fn new(git: G, process: P) -> Self {
        let agent_list_service = AgentListService::new(git.clone(), process.clone());
        Self {
            git,
            agent_list_service,
        }
    }

    pub fn cleanup(&self, all: bool) -> Result<(), GitError> {
        let all_worktrees = self.git.list_worktrees()?;
        let repo_root = self.git.get_repo_root()?;

        let active_paths_vec = self
            .agent_list_service
            .get_active_worktree_paths()
            .map_err(|_| GitError::CommandFailed("Failed to list running agents".to_string()))?;

        let mut active_paths: HashSet<PathBuf> = HashSet::new();
        for path in active_paths_vec {
            active_paths.insert(path);
        }

        let silo_dir = SiloConfig::get_silo_dir();

        let cleanable: Vec<&WorktreeInfo> = all_worktrees
            .iter()
            .filter(|wt| {
                // Exclude main worktree
                if wt.path == repo_root {
                    return false;
                }

                // Exclude worktrees with running agents
                if active_paths.contains(&wt.path) {
                    return false;
                }

                // If not --all, only include silo-managed worktrees
                if !all {
                    if let Some(ref silo) = silo_dir {
                        return wt.path.starts_with(silo);
                    }
                    return false;
                }

                true
            })
            .collect();

        if cleanable.is_empty() {
            println!("No worktrees to clean up.");
            return Ok(());
        }

        println!("Found {} worktree(s) to remove:\n", cleanable.len());
        for wt in &cleanable {
            let branch_info = wt.branch.as_deref().unwrap_or("(detached)");
            println!("  {} (branch: {})", wt.path.display(), branch_info);
        }

        let mut removed = Vec::new();
        let mut failed = Vec::new();

        println!("Removing worktrees...");
        for wt in cleanable {
            match self.git.remove_worktree(&wt.path) {
                Ok(_) => {
                    println!("  ✓ Removed {}", wt.path.display());
                    removed.push(wt.path.clone());
                }
                Err(e) => {
                    println!("  ✗ Failed to remove {}: {}", wt.path.display(), e);
                    failed.push((wt.path.clone(), e.to_string()));
                }
            }
        }

        println!("Successfully removed {} worktree(s)", removed.len());
        if !failed.is_empty() {
            println!("Failed to remove {} worktree(s)", failed.len());
        }

        Ok(())
    }
}
