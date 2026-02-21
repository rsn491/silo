use clap::ValueEnum;
use std::process::Command;
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

#[derive(Debug, Clone, PartialEq, Eq, EnumString, Display, EnumIter, IntoStaticStr, ValueEnum)]
pub enum Agent {
    #[strum(serialize = "claude")]
    #[clap(name = "claude")]
    ClaudeCode,
    #[strum(serialize = "opencode")]
    #[clap(name = "opencode")]
    OpenCode,
}

impl Agent {
    pub fn command(&self) -> Command {
        let cmd = match self {
            Agent::ClaudeCode => "claude",
            Agent::OpenCode => "opencode",
        };
        Command::new(cmd)
    }

    pub fn process_name(&self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude",
            Agent::OpenCode => "opencode",
        }
    }

    pub fn all_process_names() -> Vec<&'static str> {
        use strum::IntoEnumIterator;
        Agent::iter().map(|a| a.process_name()).collect()
    }

    pub fn all_names() -> Vec<&'static str> {
        use strum::IntoEnumIterator;
        Agent::iter().map(|a| a.into()).collect()
    }

    pub fn default() -> Self {
        Agent::ClaudeCode
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    pub fn try_from_process_name(s: &str) -> Option<Self> {
        use strum::IntoEnumIterator;
        Agent::iter().find(|agent| agent.process_name() == s)
    }
}

impl From<String> for Agent {
    fn from(s: String) -> Self {
        s.parse().unwrap_or_else(|_| panic!("Unknown agent: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_names_are_canonical() {
        assert_eq!(Agent::all_names(), vec!["claude", "opencode"]);
    }

    #[test]
    fn test_try_from_str_accepts_canonical_names() {
        assert_eq!(Agent::try_from_str("claude"), Some(Agent::ClaudeCode));
        assert_eq!(Agent::try_from_str("opencode"), Some(Agent::OpenCode));
    }

    #[test]
    fn test_try_from_str_rejects_claudecode() {
        assert_eq!(Agent::try_from_str("claudecode"), None);
    }

    #[test]
    fn test_all_process_names_are_canonical() {
        assert_eq!(Agent::all_process_names(), vec!["claude", "opencode"]);
    }

    #[test]
    fn test_try_from_process_name_accepts_canonical_names() {
        assert_eq!(
            Agent::try_from_process_name("claude"),
            Some(Agent::ClaudeCode)
        );
        assert_eq!(
            Agent::try_from_process_name("opencode"),
            Some(Agent::OpenCode)
        );
    }
}
