//! Anthropic's Claude Code agent implementation.

use super::{AgentCommand, PromptError, run_headless};

/// Concrete implementation of [`AgentCommand`] for Claude Code.
pub(super) struct ClaudeCodeAgent;

impl AgentCommand for ClaudeCodeAgent {
    fn command_name(&self) -> &'static str {
        "claude"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        run_headless("claude", &["--print", message])
    }
}
