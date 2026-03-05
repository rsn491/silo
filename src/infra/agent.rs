//! Supported AI agents and their command-line mappings.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::process::Command;
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

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
}

impl Agent {
    /// Returns a [`Command`] configured to launch this agent.
    pub fn command(&self) -> Command {
        Command::new(self.command_name())
    }

    /// Returns the executable name of the agent's command.
    pub fn command_name(&self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude",
            Agent::OpenCode => "opencode",
            Agent::Codex => "codex",
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_names_are_canonical() {
        assert_eq!(Agent::all_names(), vec!["claude", "opencode", "codex"]);
    }

    #[test]
    fn test_try_from_str_accepts_canonical_names() {
        assert_eq!(Agent::try_from_str("claude"), Some(Agent::ClaudeCode));
        assert_eq!(Agent::try_from_str("opencode"), Some(Agent::OpenCode));
        assert_eq!(Agent::try_from_str("codex"), Some(Agent::Codex));
    }

    #[test]
    fn test_try_from_str_rejects_claudecode() {
        assert_eq!(Agent::try_from_str("claudecode"), None);
    }

    #[test]
    fn test_all_command_names_are_canonical() {
        assert_eq!(
            Agent::all_command_names(),
            vec!["claude", "opencode", "codex"]
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
    }
}
