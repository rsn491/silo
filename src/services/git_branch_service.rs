//! Service for renaming auto-generated git branches with descriptive AI-generated names.

use std::path::Path;

use crate::infra::agent::Agent;
use crate::infra::git::GitOperations;
use crate::services::git_suggestions_service::GitSuggestionsService;

/// Outcome of attempting to rename an auto-generated branch.
pub enum BranchRenameOutcome {
    /// The branch was not auto-generated, had no changes, or was otherwise skipped.
    Skipped,
    /// The branch was successfully renamed to the given name.
    Renamed(String),
    /// A name was suggested but the rename operation failed.
    RenameFailed {
        /// The branch name that was suggested.
        suggested: String,
        /// The error message from the failed rename.
        error: String,
    },
    /// The agent could not produce a suggestion; contains the error message.
    SuggestionFailed(String),
}

/// Handles renaming of auto-generated git branches using AI-generated suggestions.
pub struct GitBranchService {
    /// Agent used to generate branch name suggestions.
    agent: Agent,
}

impl GitBranchService {
    /// Creates a new `GitBranchService`.
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    /// Returns `true` if `branch` looks like an auto-generated silo branch.
    ///
    /// Auto-generated branches follow the pattern `{project}-{8hexchars}`, where the suffix is
    /// the first 8 characters of a UUID (e.g., `silo-64c05006`). Any branch that still matches
    /// this pattern has not been intentionally renamed by the user.
    pub fn is_auto_generated_branch(branch: &str) -> bool {
        if let Some(suffix) = branch.rsplit('-').next() {
            suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit())
        } else {
            false
        }
    }

    /// Renames the branch at `workspace_path` to an AI-generated name if it is still auto-generated.
    ///
    /// Returns [`BranchRenameOutcome::Skipped`] if the branch has already been renamed, if no
    /// changes are found, or if the current branch cannot be determined.
    pub fn try_rename<G: GitOperations>(
        &self,
        workspace_path: &Path,
        git: &G,
    ) -> BranchRenameOutcome {
        let Ok(Some(current_branch)) = git.get_current_branch(workspace_path) else {
            return BranchRenameOutcome::Skipped;
        };
        if !Self::is_auto_generated_branch(&current_branch) {
            return BranchRenameOutcome::Skipped;
        }
        let changes = match git.get_changes_summary(workspace_path) {
            Ok(c) if !c.trim().is_empty() => c,
            _ => return BranchRenameOutcome::Skipped,
        };
        match GitSuggestionsService::new(self.agent.clone()).suggest_branch_name(&changes) {
            Err(e) => BranchRenameOutcome::SuggestionFailed(e),
            Ok(None) => BranchRenameOutcome::Skipped,
            Ok(Some(suggested)) => match git.rename_branch(workspace_path, &suggested) {
                Ok(()) => BranchRenameOutcome::Renamed(suggested),
                Err(e) => BranchRenameOutcome::RenameFailed {
                    suggested,
                    error: e.to_string(),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_auto_generated_branch_matches_silo_pattern() {
        assert!(GitBranchService::is_auto_generated_branch("silo-64c05006"));
        assert!(GitBranchService::is_auto_generated_branch(
            "myrepo-a1b2c3d4"
        ));
    }

    #[test]
    fn test_is_auto_generated_branch_rejects_user_names() {
        assert!(!GitBranchService::is_auto_generated_branch(
            "add-auth-support"
        ));
        assert!(!GitBranchService::is_auto_generated_branch("main"));
        assert!(!GitBranchService::is_auto_generated_branch("fix-login-bug"));
        // 7 hex chars — too short.
        assert!(!GitBranchService::is_auto_generated_branch("silo-64c0500"));
        // 9 hex chars — too long.
        assert!(!GitBranchService::is_auto_generated_branch(
            "silo-64c050060"
        ));
        // Non-hex suffix.
        assert!(!GitBranchService::is_auto_generated_branch("silo-xyz12345"));
    }
}
