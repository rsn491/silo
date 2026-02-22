use std::collections::HashSet;
use std::path::PathBuf;

use crate::infra::git::GitOperations;
use crate::infra::process::ProcessOperations;
use crate::services::agent_list::AgentListService;
use crate::services::agent_workspace::{AgentWorkspaceManager, CleanupError, CleanupResult};
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;

pub struct WorkspaceCleanupService<G: GitOperations + Clone, P: ProcessOperations + Clone> {
    agent_list_service: AgentListService<G, P>,
    worktree_workspace: GitWorktreeWorkspace<G>,
    checkout_workspace: GitCheckoutWorkspace<G>,
}

impl<G: GitOperations + Clone, P: ProcessOperations + Clone> WorkspaceCleanupService<G, P> {
    pub fn new(git: G, process: P) -> Self {
        Self {
            agent_list_service: AgentListService::new(git.clone(), process),
            worktree_workspace: GitWorktreeWorkspace::new(git.clone()),
            checkout_workspace: GitCheckoutWorkspace::new(git),
        }
    }

    pub fn cleanup(&self, all: bool) -> Result<CleanupResult, CleanupError> {
        let active_paths: HashSet<PathBuf> = self
            .agent_list_service
            .get_active_worktree_paths()
            .map_err(|_| CleanupError::Io("Failed to list running agents".into()))?
            .into_iter()
            .collect();

        let mut result = self.worktree_workspace.cleanup(&active_paths, all)?;
        result.extend(self.checkout_workspace.cleanup(&active_paths, all)?);

        Ok(result)
    }
}
