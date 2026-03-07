//! Codex agent implementation.

use std::process::Command;

use super::{AgentBehavior, PromptError};

/// Concrete implementation of [`AgentBehavior`] for Codex.
pub(super) struct CodexAgent;

impl AgentBehavior for CodexAgent {
    fn command_name(&self) -> &'static str {
        "codex"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        let output = Command::new("codex").args(["-q", message]).output()?;
        if !output.status.success() {
            return Err(PromptError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
