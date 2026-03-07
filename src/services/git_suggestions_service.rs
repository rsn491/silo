//! Service for generating descriptive git suggestions via an AI agent.

use std::path::Path;

use serde::Deserialize;

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
pub struct GitSuggestionsService<G: GitOperations> {
    /// Agent used to generate suggestions.
    agent: Agent,
    /// Git operations used to retrieve the changes summary.
    git: G,
}

impl<G: GitOperations> GitSuggestionsService<G> {
    /// Creates a new `GitSuggestionsService`.
    pub fn new(agent: Agent, git: G) -> Self {
        Self { agent, git }
    }

    /// Prompts the agent in headless mode with a summary of git changes and returns both a
    /// branch name suggestion and a commit message suggestion in a single call.
    ///
    /// Returns `Ok` with empty `Option` fields if there are no changes or the agent output cannot
    /// be parsed. Returns `Err` if the agent call itself fails or the changes summary cannot be
    /// retrieved.
    pub fn suggest(&self, workspace_path: &Path) -> Result<GitSuggestions, String> {
        let changes = self
            .git
            .get_changes_summary(workspace_path)
            .map_err(|e| e.to_string())?;

        if changes.trim().is_empty() {
            return Ok(GitSuggestions {
                branch_name: None,
                commit_message: None,
            });
        }

        let prompt = format!(
            "Based on the following git changes, output ONLY a JSON object and nothing else:\n\
             {{\"branch\": \"<concise kebab-case branch name, 2-5 words, lowercase letters and hyphens only, no prefixes like feature/ or fix/>\", \
             \"commit\": \"<concise commit message, imperative mood, single line>\"}}\n\n\
             Git changes:\n{}\n",
            changes
        );

        let raw = self.agent.prompt(&prompt).map_err(|e| e.to_string())?;

        #[derive(Deserialize)]
        struct Output {
            branch: Option<String>,
            commit: Option<String>,
        }

        // Extract the first JSON object from the output, tolerating any surrounding text.
        let json_str = extract_json_object(&raw).unwrap_or(raw.trim());
        let parsed: Output = serde_json::from_str(json_str).unwrap_or(Output {
            branch: None,
            commit: None,
        });

        let branch_name = parsed
            .branch
            .map(|s| sanitize_branch_name(s.trim()))
            .filter(|s| !s.is_empty());

        let commit_message = parsed
            .commit
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Ok(GitSuggestions {
            branch_name,
            commit_message,
        })
    }
}

/// Extracts the first `{...}` JSON object from `s`, returning a substring slice.
fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end >= start {
        Some(&s[start..=end])
    } else {
        None
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
