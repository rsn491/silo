//! Service for generating descriptive git branch names via an AI agent.

use crate::infra::agent::AgentCommand;

/// Generates descriptive git branch name suggestions by prompting an AI agent in headless mode.
pub struct GitSuggestionsService<A: AgentCommand> {
    /// Agent used to generate branch name suggestions.
    agent: A,
}

impl<A: AgentCommand> GitSuggestionsService<A> {
    /// Creates a new `GitSuggestionsService`.
    pub fn new(agent: A) -> Self {
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
    use crate::infra::agent::{MockAgentCommand, PromptError};

    /// An agent mock that checks the prompt mentions `changes` and replies with `reply`.
    fn agent_replying(reply: Result<&'static str, &'static str>) -> MockAgentCommand {
        let mut agent = MockAgentCommand::new();
        agent
            .expect_prompt()
            .withf(|prompt| prompt.contains("CHANGES-MARKER"))
            .times(1)
            .returning(move |_| {
                reply
                    .map(str::to_string)
                    .map_err(|e| PromptError::Failed(e.to_string()))
            });
        agent
    }

    #[test]
    fn test_suggest_branch_name_sanitizes_agent_output() {
        // Given
        let service = GitSuggestionsService::new(agent_replying(Ok("Fix Login Bug\nextra")));

        // When
        let result = service.suggest_branch_name("CHANGES-MARKER");

        // Then
        assert_eq!(result, Ok(Some("fix-login-bug".to_string())));
    }

    #[test]
    fn test_suggest_branch_name_propagates_agent_error() {
        // Given
        let service = GitSuggestionsService::new(agent_replying(Err("boom")));

        // When
        let result = service.suggest_branch_name("CHANGES-MARKER");

        // Then
        assert!(result.unwrap_err().contains("boom"));
    }

    #[test]
    fn test_suggest_commit_message_uses_first_non_empty_line() {
        // Given
        let service =
            GitSuggestionsService::new(agent_replying(Ok("\n  Add login form  \nmore detail")));

        // When
        let result = service.suggest_commit_message("CHANGES-MARKER");

        // Then
        assert_eq!(result, Ok(Some("Add login form".to_string())));
    }

    #[test]
    fn test_suggest_commit_message_returns_none_for_blank_output() {
        // Given
        let service = GitSuggestionsService::new(agent_replying(Ok(" \n \n")));

        // When
        let result = service.suggest_commit_message("CHANGES-MARKER");

        // Then
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn test_suggest_commit_message_propagates_agent_error() {
        // Given
        let service = GitSuggestionsService::new(agent_replying(Err("offline")));

        // When
        let result = service.suggest_commit_message("CHANGES-MARKER");

        // Then
        assert!(result.unwrap_err().contains("offline"));
    }

    #[test]
    fn test_sanitize_branch_name_already_kebab_case() {
        // Given
        let input = "add-auth-support";

        // When
        let result = sanitize_branch_name(input);

        // Then
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_uppercases_lowered() {
        // Given
        let input = "Add Auth Support";

        // When
        let result = sanitize_branch_name(input);

        // Then
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_special_chars_replaced() {
        // Given
        let input = "add_auth/support!";

        // When
        let result = sanitize_branch_name(input);

        // Then
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_consecutive_hyphens_collapsed() {
        // Given
        let input = "add---auth---support";

        // When
        let result = sanitize_branch_name(input);

        // Then
        assert_eq!(result.as_deref(), Some("add-auth-support"));
    }

    #[test]
    fn test_sanitize_branch_name_empty_input_returns_none() {
        // Given
        let inputs = ["", "   ", "---", "___"];

        // When
        let results: Vec<_> = inputs.iter().map(|raw| sanitize_branch_name(raw)).collect();

        // Then — inputs containing only non-alphanumeric characters all sanitize to nothing.
        assert!(results.iter().all(Option::is_none));
    }

    #[test]
    fn test_sanitize_branch_name_uses_only_first_line() {
        // Given
        let input = "add-auth\nsome other line";

        // When
        let result = sanitize_branch_name(input);

        // Then
        assert_eq!(result.as_deref(), Some("add-auth"));
    }

    #[test]
    fn test_sanitize_branch_name_skips_blank_leading_lines() {
        // Given
        let input = "\n\nadd-auth";

        // When
        let result = sanitize_branch_name(input);

        // Then
        assert_eq!(result.as_deref(), Some("add-auth"));
    }
}
