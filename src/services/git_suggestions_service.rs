//! Service for generating descriptive git suggestions via an AI agent.

use std::path::Path;

use crate::infra::agent::Agent;
use crate::infra::git::GitOperations;

/// AI-generated suggestions for a git workspace.
pub struct GitSuggestions {
    /// Suggested kebab-case branch name, or `None` if one could not be generated.
    pub branch_name: Option<String>,
    /// Suggested commit message, or `None` if one could not be generated.
    pub commit_message: Option<String>,
}

/// Generates descriptive git suggestions by prompting an AI agent in headless mode.
pub struct GitSuggestionsService {
    /// Agent used to generate suggestions.
    agent: Agent,
}

impl GitSuggestionsService {
    /// Creates a new `GitSuggestionsService`.
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    /// Prompts the agent in headless mode with a summary of git changes and returns both a
    /// branch name suggestion and a commit message suggestion in a single call.
    ///
    /// Returns `Ok` with empty `Option` fields if there are no changes or the agent output cannot
    /// be parsed. Returns `Err` if the agent call itself fails or the changes summary cannot be
    /// retrieved.
    pub fn suggest<G: GitOperations>(
        &self,
        workspace_path: &Path,
        git: &G,
    ) -> Result<GitSuggestions, String> {
        let changes = git
            .get_changes_summary(workspace_path)
            .map_err(|e| e.to_string())?;

        if changes.trim().is_empty() {
            return Ok(GitSuggestions {
                branch_name: None,
                commit_message: None,
            });
        }

        let prompt = format!(
            "Based on the following git changes, output ONLY these two lines and nothing else:\n\
             BRANCH: <concise kebab-case branch name, 2-5 words, lowercase letters and hyphens only, no prefixes like feature/ or fix/>\n\
             COMMIT: <concise commit message, imperative mood, single line>\n\n\
             Git changes:\n{}\n",
            changes
        );

        let raw = self.agent.prompt(&prompt).map_err(|e| e.to_string())?;

        let branch_name = raw
            .lines()
            .find(|l| l.trim_start().starts_with("BRANCH:"))
            .and_then(|l| l.splitn(2, ':').nth(1))
            .map(|s| sanitize_branch_name(s.trim()))
            .filter(|s| !s.is_empty());

        let commit_message = raw
            .lines()
            .find(|l| l.trim_start().starts_with("COMMIT:"))
            .and_then(|l| l.splitn(2, ':').nth(1))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Ok(GitSuggestions {
            branch_name,
            commit_message,
        })
    }
}

/// Converts `s` to a sanitized kebab-case string safe for use as a git branch name.
fn sanitize_branch_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
