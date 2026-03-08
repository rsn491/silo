//! Traits and implementations for interacting with different terminal emulators.

mod iterm2;
mod terminal_error;

use std::path::Path;

use crate::infra::agent::Agent;
pub use terminal_error::TerminalError;

pub use iterm2::ITerm2;

/// Implementation of the `Terminal` trait for detected terminal emulators.
pub type TerminalImpl = ITerm2;

/// Supported terminal emulators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalKind {
    /// iTerm2 terminal emulator.
    ITerm2,
}

/// Trait defining operations for terminal emulators.
pub trait Terminal: std::fmt::Debug {
    /// Opens a new tab in the terminal, sets the working directory, and launches the agent.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::TabOpenFailed`] if the terminal cannot open a new tab.
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError>;

    /// Splits the current pane in the terminal, sets the working directory, and launches the agent.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::PaneSplitFailed`] if the terminal cannot split the pane.
    fn split_pane(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError>;
}

/// Creates a terminal instance of the specified kind.
pub fn create_terminal(kind: &TerminalKind) -> ITerm2 {
    match kind {
        TerminalKind::ITerm2 => ITerm2,
    }
}

/// Detects and creates a terminal based on the `$TERM_PROGRAM` environment variable.
///
/// # Errors
///
/// Returns [`TerminalError::TerminalNotSupported`] if the terminal is not supported,
/// or [`TerminalError::TerminalDetectionFailed`] if the environment variable is missing.
pub fn detect_terminal() -> Result<ITerm2, TerminalError> {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let value = term_program.as_deref();

    let kind = match value.map(|s| s.to_lowercase()).as_deref() {
        Some("iterm" | "iterm2" | "iterm.app" | "iterm2.app") => TerminalKind::ITerm2,
        Some(other) => {
            return Err(TerminalError::TerminalNotSupported(format!(
                "unsupported terminal '{}'. Supported terminals: iterm",
                other
            )));
        }
        None => {
            return Err(TerminalError::TerminalDetectionFailed(
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
        // Set the environment variable for this test.
        unsafe {
            std::env::set_var("TERM_PROGRAM", "iTerm2.app");
        }
        let terminal = detect_terminal().unwrap();
        // Test that we got a terminal of the correct type.
        // We can verify it's an ITerm2 by checking it implements Debug.
        let debug_str = format!("{:?}", terminal);
        assert!(debug_str.contains("ITerm2"));
        // Clean up.
        unsafe {
            std::env::remove_var("TERM_PROGRAM");
        }
    }
}
