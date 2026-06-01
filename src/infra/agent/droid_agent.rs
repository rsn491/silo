//! Droid agent implementation.

use std::process::Command;

use super::{AgentCommand, PromptError};

/// Concrete implementation of [`AgentCommand`] for Factory.ai's Droid.
pub(super) struct DroidAgent;

impl AgentCommand for DroidAgent {
    fn command_name(&self) -> &'static str {
        "droid"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        let output = Command::new("droid").args(["exec", message]).output()?;
        if !output.status.success() {
            return Err(PromptError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
