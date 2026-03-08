//! Service for generating descriptive git branch names via an AI agent.

use crate::infra::agent::{Agent, RunningMode};
use crate::infra::git::GitOperations;
use uuid::Uuid;

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
    /// If the suggested name already exists, appends a UUID suffix.
    ///
    /// Returns `Ok(None)` if the agent output is empty or unsanitizable, and `Err` if the agent
    /// call itself fails.
    pub fn suggest_branch_name<G: GitOperations>(
        &self,
        changes: &str,
        git: &G,
    ) -> Result<Option<String>, String> {
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

        let suggested = self.prompt_and_sanitize(&prompt)?;
        match suggested {
            None => Ok(None),
            Some(name) => Ok(Some(self.ensure_unique_branch_name(name, git)?)),
        }
    }

    /// Prompts the agent with a user description and returns a sanitized branch name.
    /// If the suggested name already exists, appends a UUID suffix.
    ///
    /// Returns `Ok(None)` if the agent output is empty or unsanitizable, and `Err` if the agent
    /// call itself fails.
    pub fn suggest_branch_name_from_prompt<G: GitOperations>(
        &self,
        initial_prompt: &str,
        git: &G,
    ) -> Result<Option<String>, String> {
        let prompt = format!(
            "Based on the following description, suggest a concise, descriptive git branch name.\n\
             Rules:\n\
             - Lowercase letters and hyphens only (kebab-case)\n\
             - 2–5 words maximum\n\
             - Describes the work to be done\n\
             - No prefixes like 'feature/' or 'fix/'\n\
             - Output ONLY the branch name on a single line, nothing else\n\n\
             Description:\n{}\n\n\
             Branch name:",
            initial_prompt
        );

        let suggested = self.prompt_and_sanitize(&prompt)?;
        match suggested {
            None => Ok(None),
            Some(name) => Ok(Some(self.ensure_unique_branch_name(name, git)?)),
        }
    }

    /// Prompts the agent and returns the first sanitized kebab-case line.
    fn prompt_and_sanitize(&self, prompt: &str) -> Result<Option<String>, String> {
        let raw = self
            .agent
            .run(Some(prompt), None, RunningMode::Background, None)
            .map_err(|e| e.to_string())?;
        Ok(sanitize_branch_name(&raw))
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

        let raw = self
            .agent
            .run(Some(&prompt), None, RunningMode::Background, None)
            .map_err(|e| e.to_string())?;

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

    /// Appends a UUID suffix if the suggested branch name already exists.
    fn ensure_unique_branch_name<G: GitOperations>(
        &self,
        name: String,
        git: &G,
    ) -> Result<String, String> {
        match git.branch_exists(&name) {
            Ok(true) => Ok(format!("{}-{}", name, Uuid::new_v4())),
            Ok(false) => Ok(name),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Takes the first non-empty line of `raw`, lowercases it, replaces every non-alphanumeric
/// (non-hyphen) character with a hyphen, then collapses consecutive hyphens.
///
/// Returns `None` when the result would be empty.
fn sanitize_branch_name(raw: &str) -> Option<String> {
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
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_branch_name_already_kebab_case() {
        // Arrange
        let input = "add-auth-support";

        // Act
        let result = sanitize_branch_name(input);

        // Assert
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_uppercases_lowered() {
        // Arrange
        let input = "Add Auth Support";

        // Act
        let result = sanitize_branch_name(input);

        // Assert
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_special_chars_replaced() {
        // Arrange
        let input = "add_auth/support!";

        // Act
        let result = sanitize_branch_name(input);

        // Assert
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_consecutive_hyphens_collapsed() {
        // Arrange
        let input = "add---auth---support";

        // Act
        let result = sanitize_branch_name(input);

        // Assert
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_empty_input_returns_none() {
        // Assert — inputs containing only non-alphanumeric characters all sanitize to nothing.
        assert!(sanitize_branch_name("").is_none());
        assert!(sanitize_branch_name("   ").is_none());
        assert!(sanitize_branch_name("---").is_none());
        assert!(sanitize_branch_name("___").is_none());
    }

    #[test]
    fn test_sanitize_branch_name_uses_only_first_line() {
        // Arrange
        let input = "add-auth\nsome other line";

        // Act
        let result = sanitize_branch_name(input);

        // Assert
        assert_eq!(result.as_deref(), Some("add-auth"));
    }

    #[test]
    fn test_sanitize_branch_name_skips_blank_leading_lines() {
        // Arrange
        let input = "\n\nadd-auth";

        // Act
        let result = sanitize_branch_name(input);

        // Assert
        assert_eq!(result.as_deref(), Some("add-auth"));
    }
}
