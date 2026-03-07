//! Anthropic's Claude Code agent implementation.

use std::process::Command;

use super::{AgentBehavior, PromptError};

/// Concrete implementation of [`AgentBehavior`] for Claude Code.
pub(super) struct ClaudeCodeAgent;

impl AgentBehavior for ClaudeCodeAgent {
    fn command_name(&self) -> &'static str {
        "claude"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        let output = Command::new("claude").args(["--print", message]).output()?;
        if !output.status.success() {
            return Err(PromptError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
