//! Gemini CLI agent implementation.

use std::process::Command;

use super::{AgentCommand, AgentMode, PromptError, RunningMode};

/// Concrete implementation of [`AgentCommand`] for Google's Gemini CLI.
pub(super) struct GeminiAgent;

impl AgentCommand for GeminiAgent {
    fn command_name(&self) -> &'static str {
        "gemini"
    }

    fn run(
        &self,
        message: Option<&str>,
        _mode: Option<AgentMode>,
        exec_mode: RunningMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError> {
        let mut cmd = Command::new("gemini");
        match exec_mode {
            RunningMode::Background => {
                if let Some(msg) = message {
                    cmd.args(["-p", msg]);
                }
                if let Some(dir) = working_dir {
                    cmd.current_dir(dir);
                }
                let output = cmd.output()?;
                if !output.status.success() {
                    return Err(PromptError::Failed(
                        String::from_utf8_lossy(&output.stderr).trim().to_string(),
                    ));
                }
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            RunningMode::Foreground => {
                if let Some(msg) = message {
                    cmd.args(["-p", msg]);
                }
                if let Some(dir) = working_dir {
                    cmd.current_dir(dir);
                }
                let status = cmd.status()?;
                if !status.success() {
                    return Err(PromptError::ExitStatus(status));
                }
                Ok(String::new())
            }
        }
    }
}
