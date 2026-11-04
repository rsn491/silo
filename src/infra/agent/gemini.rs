//! Gemini CLI agent implementation.

use super::{AgentCommand, PromptError, run_headless};

/// Concrete implementation of [`AgentCommand`] for Google's Gemini CLI.
pub(super) struct GeminiAgent;

impl AgentCommand for GeminiAgent {
    fn command_name(&self) -> &'static str {
        "gemini"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        run_headless("gemini", &["-p", message])
    }
}
