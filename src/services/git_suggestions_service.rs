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
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if sanitized.is_empty() { None } else { Some(sanitized) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_branch_name_already_kebab_case() {
        assert_eq!(
            sanitize_branch_name("add-auth-support"),
            Some("add-auth-support".to_string())
        );
    }

    #[test]
    fn test_sanitize_branch_name_uppercases_lowered() {
        assert_eq!(
            sanitize_branch_name("Add Auth Support"),
            Some("add-auth-support".to_string())
        );
    }

    #[test]
    fn test_sanitize_branch_name_special_chars_replaced() {
        assert_eq!(
            sanitize_branch_name("add_auth/support!"),
            Some("add-auth-support".to_string())
        );
    }

    #[test]
    fn test_sanitize_branch_name_consecutive_hyphens_collapsed() {
        assert_eq!(
            sanitize_branch_name("add---auth---support"),
            Some("add-auth-support".to_string())
        );
    }

    #[test]
    fn test_sanitize_branch_name_empty_input_returns_none() {
        assert_eq!(sanitize_branch_name(""), None);
        assert_eq!(sanitize_branch_name("   "), None);
        assert_eq!(sanitize_branch_name("---"), None);
        assert_eq!(sanitize_branch_name("___"), None);
    }

    #[test]
    fn test_sanitize_branch_name_uses_only_first_line() {
        assert_eq!(
            sanitize_branch_name("add-auth\nsome other line"),
            Some("add-auth".to_string())
        );
    }

    #[test]
    fn test_sanitize_branch_name_skips_blank_leading_lines() {
        assert_eq!(
            sanitize_branch_name("\n\nadd-auth"),
            Some("add-auth".to_string())
        );
    }
}
