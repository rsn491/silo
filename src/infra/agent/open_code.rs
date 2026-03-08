//! OpenCode agent implementation.

use std::process::Command;

use super::{AgentCommand, PromptError};

/// Concrete implementation of [`AgentCommand`] for OpenCode.
pub(super) struct OpenCodeAgent;

impl AgentCommand for OpenCodeAgent {
    fn command_name(&self) -> &'static str {
        "opencode"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        let output = Command::new("opencode").args(["-p", message]).output()?;
        if !output.status.success() {
            return Err(PromptError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
