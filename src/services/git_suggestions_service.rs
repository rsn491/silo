//! Service for generating descriptive git branch names via an AI agent.

use crate::infra::agent::Agent;

/// Generates descriptive git branch name suggestions by prompting an AI agent in headless mode.
pub struct GitSuggestionsService {
    /// Agent used to generate branch name suggestions.
    agent: Agent,
}

impl GitSuggestionsService {
    /// Creates a new `GitSuggestionsService`.
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    /// Prompts the agent in headless mode with the git changes and returns a sanitized branch name.
    ///
    /// Returns `Ok(None)` if the agent output is empty or unsanitizable, and `Err` if the agent
    /// call itself fails.
    pub fn suggest_branch_name(&self, changes: &str) -> Result<Option<String>, String> {
        let prompt = format!(
            "Based on the following git changes, suggest a concise, descriptive git branch name.\n\
             Rules:\n\
             - Lowercase letters and hyphens only (kebab-case)\n\
             - 2–5 words maximum\n\
             - Describes the work done\n\
             - No prefixes like 'feature/' or 'fix/'\n\
             - Output ONLY the branch name on a single line, nothing else\n\n\
             Git changes:\n{}\n\n\
             Branch name:",
            changes
        );

        let raw = self.agent.prompt(&prompt).map_err(|e| e.to_string())?;

        // Take only the first non-empty line and sanitize to kebab-case.
        let sanitized: String = raw
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_lowercase()
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
            .join("-");

        if sanitized.is_empty() {
            Ok(None)
        } else {
            Ok(Some(sanitized))
        }
    }

    /// Prompts the agent in headless mode with the git changes and returns a suggested commit
    /// message.
    ///
    /// Returns `Ok(None)` if the agent output is empty, and `Err` if the agent call itself fails.
    pub fn suggest_commit_message(&self, changes: &str) -> Result<Option<String>, String> {
        let prompt = format!(
            "Based on the following git changes, suggest a concise git commit message.\n\
             Rules:\n\
             - One short sentence (imperative mood, e.g. \"Add login validation\")\n\
             - No trailing period\n\
             - Output ONLY the commit message on a single line, nothing else\n\n\
             Git changes:\n{}\n\n\
             Commit message:",
            changes
        );

        let raw = self.agent.prompt(&prompt).map_err(|e| e.to_string())?;

        let trimmed = raw
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();

        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    }
}
