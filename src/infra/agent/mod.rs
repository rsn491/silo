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
        // Given
        // (no setup needed)
        // When
        let actual = Agent::all_names();
        // Then
        assert_eq!(
            actual,
            vec!["claude", "opencode", "codex", "gemini", "droid"]
        );
    }

    #[test]
    fn test_try_from_str_accepts_canonical_names() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = Agent::try_from_str("claude");
        let actual_2 = Agent::try_from_str("opencode");
        let actual_3 = Agent::try_from_str("codex");
        let actual_4 = Agent::try_from_str("gemini");
        let actual_5 = Agent::try_from_str("droid");
        // Then
        assert_eq!(actual_1, Some(Agent::ClaudeCode));
        assert_eq!(actual_2, Some(Agent::OpenCode));
        assert_eq!(actual_3, Some(Agent::Codex));
        assert_eq!(actual_4, Some(Agent::Gemini));
        assert_eq!(actual_5, Some(Agent::Droid));
    }

    #[test]
    fn test_try_from_str_rejects_claudecode() {
        // Given
        // (no setup needed)
        // When
        let actual = Agent::try_from_str("claudecode");
        // Then
        assert_eq!(actual, None);
    }

    #[test]
    fn test_all_command_names_are_canonical() {
        // Given
        // (no setup needed)
        // When
        let actual = Agent::all_command_names();
        // Then
        assert_eq!(
            actual,
            vec!["claude", "opencode", "codex", "gemini", "droid"]
        );
    }

    #[test]
    fn test_try_from_command_name_accepts_canonical_names() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = Agent::try_from_command_name("claude");
        let actual_2 = Agent::try_from_command_name("opencode");
        let actual_3 = Agent::try_from_command_name("codex");
        let actual_4 = Agent::try_from_command_name("gemini");
        let actual_5 = Agent::try_from_command_name("droid");
        // Then
        assert_eq!(actual_1, Some(Agent::ClaudeCode));
        assert_eq!(actual_2, Some(Agent::OpenCode));
        assert_eq!(actual_3, Some(Agent::Codex));
        assert_eq!(actual_4, Some(Agent::Gemini));
        assert_eq!(actual_5, Some(Agent::Droid));
    }

    #[test]
    fn test_agent_behavior_delegation() {
        // Given
        let codex_command = Agent::Codex.process();
        // When
        let actual_1 = Agent::ClaudeCode.command_name();
        let actual_2 = Agent::OpenCode.command_name();
        let actual_3 = Agent::Codex.command_name();
        let actual_4 = Agent::Gemini.command_name();
        let actual_5 = Agent::Droid.command_name();
        let actual_6 = AgentCommand::command_name(&Agent::Gemini);
        let actual_7 = codex_command.get_program();
        // Then
        assert_eq!(actual_1, "claude");
        assert_eq!(actual_2, "opencode");
        assert_eq!(actual_3, "codex");
        assert_eq!(actual_4, "gemini");
        assert_eq!(actual_5, "droid");
        assert_eq!(actual_6, "gemini");
        assert_eq!(actual_7, "codex");
    }

    #[test]
    fn test_default_agent_is_claude() {
        // Given
        // (no setup needed)
        // When
        let actual = Agent::default();
        // Then
        assert_eq!(actual, Agent::ClaudeCode);
    }

    #[test]
    fn test_reference_delegates_to_agent_command() {
        // Given
        let mut mock = MockAgentCommand::new();
        mock.expect_command_name().return_const("mock");
        mock.expect_prompt()
            .returning(|message| Ok(format!("echo: {}", message)));
        let reference = &mock;
        // When
        let actual_1 = AgentCommand::command_name(&reference);
        let actual_2 = AgentCommand::prompt(&reference, "hi").unwrap();
        // Then
        assert_eq!(actual_1, "mock");
        assert_eq!(actual_2, "echo: hi");
    }

    #[test]
    fn test_run_headless_returns_trimmed_stdout() {
        // Given
        // (no setup needed)
        // When
        let output = run_headless("sh", &["-c", "printf '  hello \\n'"]).unwrap();
        // Then
        assert_eq!(output, "hello");
    }

    #[test]
    fn test_run_headless_reports_stderr_on_failure() {
        // Given
        // (no setup needed)
        // When
        let result = run_headless("sh", &["-c", "echo boom >&2; exit 3"]);
        // Then
        assert!(matches!(result, Err(PromptError::Failed(msg)) if msg == "boom"));
    }

    #[test]
    fn test_run_headless_reports_spawn_failure() {
        // Given
        let result = run_headless("silo-definitely-missing-binary", &[]);
        // When
        let err = result.unwrap_err();
        // Then
        assert!(matches!(err, PromptError::Io(_)));
        assert!(err.to_string().starts_with("failed to spawn agent"));
    }
}
