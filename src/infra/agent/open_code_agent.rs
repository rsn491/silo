//! OpenCode agent implementation.

use std::process::Command;

use super::{AgentCommand, AgentMode, PromptError, RunningMode};

/// Concrete implementation of [`AgentCommand`] for OpenCode.
pub(super) struct OpenCodeAgent;

impl AgentCommand for OpenCodeAgent {
    fn command_name(&self) -> &'static str {
        "opencode"
    }

    fn mode_args(&self, mode: AgentMode) -> &'static [&'static str] {
        match mode {
            AgentMode::Plan => &["--agent", "plan"],
            AgentMode::Code => &[],
        }
    }

    fn run(
        &self,
        message: Option<&str>,
        mode: Option<AgentMode>,
        exec_mode: RunningMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError> {
        match exec_mode {
            RunningMode::Background => {
                // `opencode run [message]` is the non-interactive subcommand.
                let mut cmd = Command::new("opencode");
                cmd.arg("run");
                cmd.args(self.mode_args(mode.unwrap_or(AgentMode::Code)));
                if let Some(msg) = message {
                    cmd.arg(msg);
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
                let mut cmd = Command::new("opencode");
                cmd.args(self.mode_args(mode.unwrap_or(AgentMode::Code)));
                if let Some(msg) = message {
                    cmd.args(["--prompt", msg]);
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
