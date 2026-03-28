//! Codex agent implementation.

use std::process::Command;

use super::{AgentCommand, AgentMode, PromptError, RunningMode};

/// Concrete implementation of [`AgentCommand`] for Codex.
pub(super) struct CodexAgent;

impl AgentCommand for CodexAgent {
    fn command_name(&self) -> &'static str {
        "codex"
    }

    fn mode_args(&self, mode: AgentMode) -> &'static [&'static str] {
        match mode {
            AgentMode::Plan => &["--sandbox", "read-only"],
            AgentMode::Code => &[],
        }
    }

    fn supports_mode(&self, mode: AgentMode) -> bool {
        mode != AgentMode::Plan
    }

    fn run(
        &self,
        message: Option<&str>,
        mode: Option<AgentMode>,
        exec_mode: RunningMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError> {
        let mut cmd = Command::new("codex");
        cmd.args(self.mode_args(mode.unwrap_or(AgentMode::Code)));
        match exec_mode {
            RunningMode::Background => {
                if let Some(msg) = message {
                    cmd.args(["-q", msg]);
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
                    cmd.arg(msg);
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
