//! Codex agent implementation.

use std::process::Command;

use super::{AgentCommand, AgentMode, PromptError};

/// Concrete implementation of [`AgentCommand`] for Codex.
pub(super) struct CodexAgent;

impl AgentCommand for CodexAgent {
    fn command_name(&self) -> &'static str {
        "codex"
    }

    fn prompt(&self, message: &str, mode: Option<AgentMode>) -> Result<String, PromptError> {
        let mut args = vec!["-q"];
        if let Some(mode) = mode {
            args.push(match mode {
                AgentMode::Plan => "--plan",
                AgentMode::Code => "--code",
            });
        }
        args.push(message);

        let output = Command::new("codex").args(args).output()?;
        if !output.status.success() {
            return Err(PromptError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
