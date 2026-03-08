//! Supported AI agents and their command-line mappings.

mod claude_code_agent;
mod codex_agent;
mod gemini;
mod open_code_agent;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};
use thiserror::Error;

use claude_code_agent::ClaudeCodeAgent;
use codex_agent::CodexAgent;
use gemini::GeminiAgent;
use open_code_agent::OpenCodeAgent;

/// The interaction mode used when launching the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentMode {
    /// Standard code-writing mode (default).
    #[default]
    Code,
    /// Planning mode: the agent is invoked with its native plan-mode flag.
    Plan,
}

impl AgentMode {
    /// Returns the display label for this mode.
    pub fn display_name(self) -> &'static str {
        match self {
            AgentMode::Code => "Code",
            AgentMode::Plan => "Plan",
        }
    }

    /// Toggles between `Code` and `Plan`.
    pub fn toggle(self) -> Self {
        match self {
            AgentMode::Code => AgentMode::Plan,
            AgentMode::Plan => AgentMode::Code,
        }
    }
}

/// Errors that can occur when prompting an agent.
#[derive(Debug, Error)]
pub enum PromptError {
    /// The agent process could not be spawned.
    #[error("failed to spawn agent: {0}")]
    Io(#[from] std::io::Error),
    /// The agent exited with a non-zero status (headless mode).
    #[error("agent prompt failed: {0}")]
    Failed(String),
    /// The agent exited with a non-zero status (foreground mode).
    #[error("agent exited with status: {0}")]
    ExitStatus(std::process::ExitStatus),
}

/// Whether the agent is invoked interactively (foreground) or non-interactively (headless).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// Run the agent interactively with inherited stdio; output is not captured.
    Foreground,
    /// Run the agent non-interactively and capture its stdout output.
    Headless,
}

/// Behavior that every agent variant must implement.
pub trait AgentCommand {
    /// Returns the executable name used to invoke this agent.
    fn command_name(&self) -> &'static str;

    /// Returns the CLI arguments required to invoke the agent in the given mode.
    ///
    /// The default implementation returns an empty slice (no extra args needed for `Code` mode).
    /// Agents that support a native plan mode should override this for `AgentMode::Plan`.
    fn mode_args(&self, mode: AgentMode) -> &'static [&'static str] {
        let _ = mode;
        &[]
    }

    /// Sends a prompt to the agent in the specified execution mode.
    ///
    /// - [`PromptMode::Headless`]: runs the agent non-interactively and returns captured stdout.
    /// - [`PromptMode::Foreground`]: runs the agent interactively with inherited stdio; returns
    ///   an empty string on success.
    ///
    /// `message` is optional for foreground mode (the agent launches without an initial prompt
    /// when `None`). For headless mode `message` should always be `Some`.
    ///
    /// `working_dir` sets the working directory for the spawned process. Pass `None` to inherit
    /// the current directory.
    ///
    /// # Errors
    ///
    /// Returns [`PromptError::Io`] if the process cannot be spawned,
    /// [`PromptError::Failed`] if a headless agent exits with a non-zero status, or
    /// [`PromptError::ExitStatus`] if a foreground agent exits with a non-zero status.
    fn prompt(
        &self,
        message: Option<&str>,
        mode: Option<AgentMode>,
        exec_mode: PromptMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError>;
}

/// Represents the supported AI agents that can be launched.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    EnumString,
    Display,
    EnumIter,
    IntoStaticStr,
    ValueEnum,
    Serialize,
    Deserialize,
    Default,
)]
pub enum Agent {
    /// Anthropic's Claude Code agent.
    #[strum(serialize = "claude")]
    #[clap(name = "claude")]
    #[serde(rename = "claude")]
    #[default]
    ClaudeCode,
    /// The OpenCode agent.
    #[strum(serialize = "opencode")]
    #[clap(name = "opencode")]
    #[serde(rename = "opencode")]
    OpenCode,
    /// The Codex agent.
    #[strum(serialize = "codex")]
    #[clap(name = "codex")]
    #[serde(rename = "codex")]
    Codex,
    /// Google's Gemini CLI agent.
    #[strum(serialize = "gemini")]
    #[clap(name = "gemini")]
    #[serde(rename = "gemini")]
    Gemini,
}

impl Agent {
    /// Returns the per-variant struct that implements [`AgentCommand`].
    fn command(&self) -> &dyn AgentCommand {
        match self {
            Agent::ClaudeCode => &ClaudeCodeAgent,
            Agent::OpenCode => &OpenCodeAgent,
            Agent::Codex => &CodexAgent,
            Agent::Gemini => &GeminiAgent,
        }
    }

    /// Returns the executable name of the agent's command.
    pub fn command_name(&self) -> &'static str {
        self.command().command_name()
    }

    /// Sends a prompt to the agent in the specified execution mode.
    ///
    /// See [`AgentCommand::prompt`] for full documentation.
    ///
    /// # Errors
    ///
    /// Returns [`PromptError`] if the process cannot be spawned or exits with a non-zero status.
    pub fn prompt(
        &self,
        message: Option<&str>,
        mode: Option<AgentMode>,
        exec_mode: PromptMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError> {
        self.command().prompt(message, mode, exec_mode, working_dir)
    }

    /// Returns the CLI arguments for the given launch mode.
    pub fn mode_args(&self, mode: AgentMode) -> &'static [&'static str] {
        self.command().mode_args(mode)
    }

    /// Returns a [`Command`] configured to launch this agent in the given mode.
    pub fn process_for_mode(&self, mode: AgentMode) -> std::process::Command {
        let mut cmd = std::process::Command::new(self.command_name());
        cmd.args(self.mode_args(mode));
        cmd
    }

    /// Returns a [`Command`] configured to launch this agent.
    pub fn process(&self) -> std::process::Command {
        std::process::Command::new(self.command_name())
    }

    /// Returns a list of command names for all supported agents.
    pub fn all_command_names() -> Vec<&'static str> {
        use strum::IntoEnumIterator;
        Agent::iter().map(|a| a.command_name()).collect()
    }

    /// Returns a list of canonical names for all supported agents.
    pub fn all_names() -> Vec<&'static str> {
        use strum::IntoEnumIterator;
        Agent::iter().map(|a| a.into()).collect()
    }

    /// Returns the default agent (ClaudeCode).
    pub fn default() -> Self {
        Agent::ClaudeCode
    }

    /// Attempts to parse an agent from its canonical name.
    pub fn try_from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// Attempts to find an agent by its command name.
    pub fn try_from_command_name(s: &str) -> Option<Self> {
        use strum::IntoEnumIterator;
        Agent::iter().find(|agent| agent.command_name() == s)
    }
}

impl AgentCommand for Agent {
    fn command_name(&self) -> &'static str {
        self.command().command_name()
    }

    fn prompt(
        &self,
        message: Option<&str>,
        mode: Option<AgentMode>,
        exec_mode: PromptMode,
        working_dir: Option<&std::path::Path>,
    ) -> Result<String, PromptError> {
        self.command().prompt(message, mode, exec_mode, working_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_names_are_canonical() {
        assert_eq!(
            Agent::all_names(),
            vec!["claude", "opencode", "codex", "gemini"]
        );
    }

    #[test]
    fn test_try_from_str_accepts_canonical_names() {
        assert_eq!(Agent::try_from_str("claude"), Some(Agent::ClaudeCode));
        assert_eq!(Agent::try_from_str("opencode"), Some(Agent::OpenCode));
        assert_eq!(Agent::try_from_str("codex"), Some(Agent::Codex));
        assert_eq!(Agent::try_from_str("gemini"), Some(Agent::Gemini));
    }

    #[test]
    fn test_try_from_str_rejects_claudecode() {
        assert_eq!(Agent::try_from_str("claudecode"), None);
    }

    #[test]
    fn test_all_command_names_are_canonical() {
        assert_eq!(
            Agent::all_command_names(),
            vec!["claude", "opencode", "codex", "gemini"]
        );
    }

    #[test]
    fn test_try_from_command_name_accepts_canonical_names() {
        assert_eq!(
            Agent::try_from_command_name("claude"),
            Some(Agent::ClaudeCode)
        );
        assert_eq!(
            Agent::try_from_command_name("opencode"),
            Some(Agent::OpenCode)
        );
        assert_eq!(Agent::try_from_command_name("codex"), Some(Agent::Codex));
        assert_eq!(Agent::try_from_command_name("gemini"), Some(Agent::Gemini));
    }

    #[test]
    fn test_agent_behavior_delegation() {
        assert_eq!(Agent::ClaudeCode.command_name(), "claude");
        assert_eq!(Agent::OpenCode.command_name(), "opencode");
        assert_eq!(Agent::Codex.command_name(), "codex");
    }
}
