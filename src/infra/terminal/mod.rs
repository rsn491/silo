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
    detect_terminal_from_value(std::env::var("TERM_PROGRAM").ok().as_deref())
}

/// Core terminal detection logic, accepting the `TERM_PROGRAM` value directly.
///
/// Separated from [`detect_terminal`] so tests can exercise the matching logic without
/// mutating process-global environment variables.
fn detect_terminal_from_value(term_program: Option<&str>) -> Result<ITerm2, TerminalError> {
    let kind = match term_program.map(|s| s.to_lowercase()).as_deref() {
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
    fn test_detect_terminal_iterm2_app_spelling() {
        // Arrange
        let term_program = Some("iTerm2.app");

        // Act
        let terminal = detect_terminal_from_value(term_program)
            .expect("iTerm2.app should be detected as iTerm2");

        // Assert
        assert!(format!("{terminal:?}").contains("ITerm2"));
    }

    #[test]
    fn test_detect_terminal_iterm2_variant_names() {
        // Assert — all documented TERM_PROGRAM spellings for iTerm2 are accepted.
        for value in &["iterm", "iterm2", "iterm.app", "iterm2.app", "iTerm2"] {
            let result = detect_terminal_from_value(Some(value));
            assert!(
                result.is_ok(),
                "TERM_PROGRAM={value:?} should be recognised as iTerm2"
            );
        }
    }

    #[test]
    fn test_detect_terminal_unsupported_terminal_returns_error() {
        // Arrange
        let term_program = Some("gnome-terminal");

        // Act
        let result = detect_terminal_from_value(term_program);

        // Assert
        let err = result.expect_err("unsupported terminal should yield an error");
        assert!(
            matches!(err, TerminalError::TerminalNotSupported(_)),
            "expected TerminalNotSupported, got {err:?}"
        );
    }

    #[test]
    fn test_detect_terminal_missing_env_var_returns_error() {
        // Act
        let result = detect_terminal_from_value(None);

        // Assert
        let err = result.expect_err("missing TERM_PROGRAM should yield an error");
        assert!(
            matches!(err, TerminalError::TerminalDetectionFailed(_)),
            "expected TerminalDetectionFailed, got {err:?}"
        );
    }
}
