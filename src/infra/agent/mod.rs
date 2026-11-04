//! Supported AI agents and their command-line mappings.

mod claude_code_agent;
mod codex_agent;
mod droid_agent;
mod gemini;
mod open_code_agent;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};
use thiserror::Error;

use claude_code_agent::ClaudeCodeAgent;
use codex_agent::CodexAgent;
use droid_agent::DroidAgent;
use gemini::GeminiAgent;
use open_code_agent::OpenCodeAgent;

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
#[cfg_attr(test, mockall::automock)]
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

impl<T: AgentCommand + ?Sized> AgentCommand for &T {
    fn command_name(&self) -> &'static str {
        (**self).command_name()
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        (**self).prompt(message)
    }
}

/// Runs `program` with `args` non-interactively and returns its trimmed stdout.
///
/// # Errors
///
/// Returns [`PromptError::Io`] if the process cannot be spawned, or [`PromptError::Failed`]
/// with the trimmed stderr if it exits with a non-zero status.
fn run_headless(program: &str, args: &[&str]) -> Result<String, PromptError> {
    let output = std::process::Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(PromptError::Failed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
    /// The Droid agent from factory.ai.
    #[strum(serialize = "droid")]
    #[clap(name = "droid")]
    #[serde(rename = "droid")]
    Droid,
}

impl Agent {
    /// Returns the per-variant struct that implements [`AgentCommand`].
    fn command(&self) -> &dyn AgentCommand {
        match self {
            Agent::ClaudeCode => &ClaudeCodeAgent,
            Agent::OpenCode => &OpenCodeAgent,
            Agent::Codex => &CodexAgent,
            Agent::Gemini => &GeminiAgent,
            Agent::Droid => &DroidAgent,
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
    /// - Droid: `-p`
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
            vec!["claude", "opencode", "codex", "gemini", "droid"]
        );
    }

    #[test]
    fn test_try_from_str_accepts_canonical_names() {
        assert_eq!(Agent::try_from_str("claude"), Some(Agent::ClaudeCode));
        assert_eq!(Agent::try_from_str("opencode"), Some(Agent::OpenCode));
        assert_eq!(Agent::try_from_str("codex"), Some(Agent::Codex));
        assert_eq!(Agent::try_from_str("gemini"), Some(Agent::Gemini));
        assert_eq!(Agent::try_from_str("droid"), Some(Agent::Droid));
    }

    #[test]
    fn test_try_from_str_rejects_claudecode() {
        assert_eq!(Agent::try_from_str("claudecode"), None);
    }

    #[test]
    fn test_all_command_names_are_canonical() {
        assert_eq!(
            Agent::all_command_names(),
            vec!["claude", "opencode", "codex", "gemini", "droid"]
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
        assert_eq!(Agent::try_from_command_name("droid"), Some(Agent::Droid));
    }

    #[test]
    fn test_agent_behavior_delegation() {
        assert_eq!(Agent::ClaudeCode.command_name(), "claude");
        assert_eq!(Agent::OpenCode.command_name(), "opencode");
        assert_eq!(Agent::Codex.command_name(), "codex");
        assert_eq!(Agent::Gemini.command_name(), "gemini");
        assert_eq!(Agent::Droid.command_name(), "droid");
        assert_eq!(AgentCommand::command_name(&Agent::Gemini), "gemini");
        assert_eq!(Agent::Codex.process().get_program(), "codex");
    }

    #[test]
    fn test_default_agent_is_claude() {
        assert_eq!(Agent::default(), Agent::ClaudeCode);
    }

    #[test]
    fn test_reference_delegates_to_agent_command() {
        let mut mock = MockAgentCommand::new();
        mock.expect_command_name().return_const("mock");
        mock.expect_prompt()
            .returning(|message| Ok(format!("echo: {}", message)));
        let reference = &mock;

        assert_eq!(AgentCommand::command_name(&reference), "mock");
        assert_eq!(AgentCommand::prompt(&reference, "hi").unwrap(), "echo: hi");
    }

    #[test]
    fn test_run_headless_returns_trimmed_stdout() {
        let output = run_headless("sh", &["-c", "printf '  hello \\n'"]).unwrap();

        assert_eq!(output, "hello");
    }

    #[test]
    fn test_run_headless_reports_stderr_on_failure() {
        let result = run_headless("sh", &["-c", "echo boom >&2; exit 3"]);

        assert!(matches!(result, Err(PromptError::Failed(msg)) if msg == "boom"));
    }

    #[test]
    fn test_run_headless_reports_spawn_failure() {
        let result = run_headless("silo-definitely-missing-binary", &[]);

        let err = result.unwrap_err();
        assert!(matches!(err, PromptError::Io(_)));
        assert!(err.to_string().starts_with("failed to spawn agent"));
    }
}
