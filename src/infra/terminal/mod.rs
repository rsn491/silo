mod iterm2;

use std::path::Path;

use crate::infra::agent::Agent;
use crate::services::agent_launcher::LaunchError;

pub use iterm2::ITerm2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalKind {
    ITerm2,
}

pub trait Terminal: std::fmt::Debug {
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), LaunchError>;
    fn split_pane(&self, worktree_path: &Path, agent: &Agent) -> Result<(), LaunchError>;
}

pub fn create_terminal(kind: &TerminalKind) -> ITerm2 {
    match kind {
        TerminalKind::ITerm2 => ITerm2,
    }
}

/// Detect and create a terminal from `$TERM_PROGRAM`.
pub fn detect_terminal() -> Result<ITerm2, LaunchError> {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let value = term_program.as_deref();

    let kind = match value.map(|s| s.to_lowercase()).as_deref() {
        Some("iterm" | "iterm2" | "iterm.app" | "iterm2.app") => TerminalKind::ITerm2,
        Some(other) => {
            return Err(LaunchError::AgentSpawnError(format!(
                "unsupported terminal '{}'. Supported terminals: iterm",
                other
            )));
        }
        None => {
            return Err(LaunchError::AgentSpawnError(
                "could not detect terminal. Set $TERM_PROGRAM. \
                 Supported terminals: iterm"
                    .to_string(),
            ));
        }
    };

    Ok(create_terminal(&kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_terminal_auto_from_term_program() {
        // Set the environment variable for this test
        unsafe {
            std::env::set_var("TERM_PROGRAM", "iTerm2.app");
        }
        let terminal = detect_terminal().unwrap();
        // Test that we got a terminal of the correct type
        // We can verify it's an ITerm2 by checking it implements Debug
        let debug_str = format!("{:?}", terminal);
        assert!(debug_str.contains("ITerm2"));
        // Clean up
        unsafe {
            std::env::remove_var("TERM_PROGRAM");
        }
    }
}
