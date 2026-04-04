//! Traits and implementations for interacting with different terminal emulators.

mod iterm2;
mod terminal_error;

use std::path::Path;

use crate::infra::agent::Agent;
pub use terminal_error::TerminalError;

pub use iterm2::ITerm2;

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

    /// Splits the current pane in the terminal and runs `command` from `working_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::PaneSplitFailed`] if the terminal cannot split the pane.
    fn split_pane(&self, working_dir: &Path, command: &str) -> Result<(), TerminalError>;
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
        // Arrange
        // Set the environment variable for this test.
        unsafe {
            std::env::set_var("TERM_PROGRAM", "iTerm2.app");
        }

        // Act
        let terminal =
            detect_terminal().expect("should detect iTerm2 from TERM_PROGRAM=iTerm2.app");

        // Assert
        // Test that we got a terminal of the correct type.
        // We can verify it's an ITerm2 by checking it implements Debug.
        let debug_str = format!("{:?}", terminal);
        assert!(debug_str.contains("ITerm2"));

        // Clean up.
        unsafe {
            std::env::remove_var("TERM_PROGRAM");
        }
    }

    #[test]
    fn test_detect_terminal_iterm2_variant_names() {
        // All known iTerm2 TERM_PROGRAM values should be accepted.
        for value in &["iterm", "iterm2", "iterm.app", "iterm2.app", "iTerm2"] {
            unsafe {
                std::env::set_var("TERM_PROGRAM", value);
            }
            let result = detect_terminal();
            unsafe {
                std::env::remove_var("TERM_PROGRAM");
            }
            assert!(
                result.is_ok(),
                "TERM_PROGRAM={value:?} should be recognised as iTerm2"
            );
        }
    }

    #[test]
    fn test_detect_terminal_unsupported_terminal_returns_error() {
        unsafe {
            std::env::set_var("TERM_PROGRAM", "gnome-terminal");
        }

        let result = detect_terminal();

        unsafe {
            std::env::remove_var("TERM_PROGRAM");
        }

        assert!(
            result.is_err(),
            "unsupported terminal should yield an error"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, TerminalError::TerminalNotSupported(_)),
            "expected TerminalNotSupported, got {:?}",
            err
        );
    }
}
