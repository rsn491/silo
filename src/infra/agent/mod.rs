//! Supported AI agents and their command-line mappings.

mod claude_code;
mod codex;
mod gemini;
mod open_code;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};
use thiserror::Error;

use claude_code::ClaudeCodeAgent;
use codex::CodexAgent;
use gemini::GeminiAgent;
use open_code::OpenCodeAgent;

/// Errors that can occur when prompting an agent in headless mode.
#[derive(Debug, Error)]
pub enum PromptError {
    /// The agent process could not be spawned.
    #[error("failed to spawn agent: {0}")]
    Io(#[from] std::io::Error),
    /// The agent exited with a non-zero status.
    #[error("agent prompt failed: {0}")]
    Failed(String),
}

/// Behavior that every agent variant must implement.
pub trait AgentCommand {
    /// Returns the executable name used to invoke this agent.
    fn command_name(&self) -> &'static str;

    /// Runs the agent in headless mode with the given prompt and returns captured stdout.
    ///
    /// # Errors
    ///
    /// Returns [`PromptError::Io`] if the process cannot be spawned, or [`PromptError::Failed`]
    /// if the agent exits with a non-zero status.
    fn prompt(&self, message: &str) -> Result<String, PromptError>;
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

    /// Runs the agent in headless (non-interactive) mode with the given prompt and returns the
    /// captured stdout output.
    ///
    /// Each agent uses its own flag convention for headless/print mode:
    /// - Claude Code: `--print`
    /// - OpenCode: `-p`
    /// - Codex: `-q`
    /// - Gemini: `-p`
    ///
    /// # Errors
    ///
    /// Returns [`PromptError::Io`] if the process cannot be spawned, or [`PromptError::Failed`]
    /// if the agent exits with a non-zero status.
    pub fn prompt(&self, message: &str) -> Result<String, PromptError> {
        self.command().prompt(message)
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

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        self.command().prompt(message)
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
