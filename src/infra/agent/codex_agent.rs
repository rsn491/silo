//! Codex agent implementation.

use super::{AgentCommand, PromptError, run_headless};

/// Concrete implementation of [`AgentCommand`] for Codex.
pub(super) struct CodexAgent;

impl AgentCommand for CodexAgent {
    fn command_name(&self) -> &'static str {
        "codex"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        run_headless("codex", &["-q", message])
    }
}
