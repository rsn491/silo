//! Anthropic's Claude Code agent implementation.

use std::process::Command;

use super::{AgentCommand, AgentMode, PromptError, PromptMode};

/// Concrete implementation of [`AgentCommand`] for Claude Code.
pub(super) struct ClaudeCodeAgent;

impl AgentCommand for ClaudeCodeAgent {
    fn command_name(&self) -> &'static str {
        "claude"
    }

    fn mode_args(&self, mode: AgentMode) -> &'static [&'static str] {
        match mode {
            AgentMode::Plan => &["--permission-mode", "plan"],
            AgentMode::Code => &[],
        }
    }

    fn prompt(
        &self,
        message: Option<&str>,
        mode: Option<AgentMode>,
        exec_mode: PromptMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError> {
        let mut cmd = Command::new("claude");
        cmd.args(self.mode_args(mode.unwrap_or(AgentMode::Code)));
        match exec_mode {
            PromptMode::Headless => {
                if let Some(msg) = message {
                    cmd.args(["--print", msg]);
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
            PromptMode::Foreground => {
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
