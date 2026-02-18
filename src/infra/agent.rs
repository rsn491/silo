use clap::ValueEnum;
use std::process::Command;
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

#[derive(Debug, Clone, PartialEq, Eq, EnumString, Display, EnumIter, IntoStaticStr, ValueEnum)]
#[strum(serialize_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum Agent {
    ClaudeCode,
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
}

impl From<String> for Agent {
    fn from(s: String) -> Self {
        s.parse().unwrap_or_else(|_| panic!("Unknown agent: {}", s))
    }
}
