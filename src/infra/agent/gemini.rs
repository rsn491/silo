//! Gemini CLI agent implementation.

use std::process::Command;

use super::{AgentCommand, PromptError};

/// Concrete implementation of [`AgentCommand`] for Google's Gemini CLI.
pub(super) struct GeminiAgent;

impl AgentCommand for GeminiAgent {
    fn command_name(&self) -> &'static str {
        "gemini"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        let output = Command::new("gemini").args(["-p", message]).output()?;
        if !output.status.success() {
            return Err(PromptError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
