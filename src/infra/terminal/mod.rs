//! Traits and implementations for interacting with different terminal emulators.

mod command;
mod ghostty;
mod iterm2;
mod kitty;
mod terminal_error;
mod wezterm;

use std::path::Path;

use crate::infra::agent::Agent;
pub use terminal_error::TerminalError;

pub use ghostty::Ghostty;
pub use iterm2::ITerm2;
pub use kitty::Kitty;
pub use wezterm::WezTerm;

/// Supported terminal emulators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalKind {
    /// iTerm2 terminal emulator.
    ITerm2,
    /// Ghostty terminal emulator.
    Ghostty,
    /// kitty terminal emulator.
    Kitty,
    /// WezTerm terminal emulator.
    WezTerm,
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

/// Forwarding implementation so a boxed trait object satisfies `T: Terminal` bounds,
/// allowing the launch path to select a terminal at runtime.
impl Terminal for Box<dyn Terminal> {
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        (**self).open_tab(worktree_path, agent)
    }

    fn split_pane(&self, working_dir: &Path, command: &str) -> Result<(), TerminalError> {
        (**self).split_pane(working_dir, command)
    }
}

/// Human-readable list of supported terminals, used in detection error messages.
const SUPPORTED_TERMINALS: &str = "iterm, ghostty, kitty, wezterm";

/// Creates a terminal instance of the specified kind.
pub fn create_terminal(kind: &TerminalKind) -> Box<dyn Terminal> {
    match kind {
        TerminalKind::ITerm2 => Box::new(ITerm2),
        TerminalKind::Ghostty => Box::new(Ghostty),
        TerminalKind::Kitty => Box::new(Kitty),
        TerminalKind::WezTerm => Box::new(WezTerm),
    }
}

/// Detects and creates a terminal based on the current environment.
///
/// Reads `$TERM_PROGRAM` (iTerm2, Ghostty, WezTerm) and falls back to `$TERM` /
/// `$KITTY_WINDOW_ID` to detect kitty, which does not set `$TERM_PROGRAM`.
///
/// # Errors
///
/// Returns [`TerminalError::TerminalNotSupported`] if the terminal is not supported,
/// or [`TerminalError::TerminalDetectionFailed`] if no terminal could be identified.
pub fn detect_terminal() -> Result<Box<dyn Terminal>, TerminalError> {
    detect_terminal_from_env(
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
        std::env::var("KITTY_WINDOW_ID").ok().as_deref(),
    )
}

/// Core terminal detection logic, accepting the relevant environment values directly.
///
/// Separated from [`detect_terminal`] so tests can exercise the matching logic without
/// mutating process-global environment variables.
fn detect_terminal_from_env(
    term_program: Option<&str>,
    term: Option<&str>,
    kitty_window_id: Option<&str>,
) -> Result<Box<dyn Terminal>, TerminalError> {
    let kind = match term_program.map(|s| s.to_lowercase()).as_deref() {
        Some("iterm" | "iterm2" | "iterm.app" | "iterm2.app") => TerminalKind::ITerm2,
        Some("ghostty" | "ghostty.app") => TerminalKind::Ghostty,
        Some("wezterm") => TerminalKind::WezTerm,
        Some(other) => {
            return Err(TerminalError::TerminalNotSupported(format!(
                "unsupported terminal '{}'. Supported terminals: {}",
                other, SUPPORTED_TERMINALS
            )));
        }
        // kitty does not set $TERM_PROGRAM; detect it via $TERM / $KITTY_WINDOW_ID.
        None if term == Some("xterm-kitty") || kitty_window_id.is_some() => TerminalKind::Kitty,
        None => {
            return Err(TerminalError::TerminalDetectionFailed(format!(
                "could not detect terminal. Set $TERM_PROGRAM. Supported terminals: {}",
                SUPPORTED_TERMINALS
            )));
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
        let terminal = detect_terminal_from_env(term_program, None, None)
            .expect("iTerm2.app should be detected as iTerm2");

        // Assert
        assert!(format!("{terminal:?}").contains("ITerm2"));
    }

    #[test]
    fn test_detect_terminal_iterm2_variant_names() {
        // Assert — all documented TERM_PROGRAM spellings for iTerm2 are accepted.
        for value in &["iterm", "iterm2", "iterm.app", "iterm2.app", "iTerm2"] {
            let result = detect_terminal_from_env(Some(value), None, None);
            assert!(
                result.is_ok(),
                "TERM_PROGRAM={value:?} should be recognised as iTerm2"
            );
        }
    }

    #[test]
    fn test_detect_terminal_ghostty() {
        // Assert — Ghostty is detected via TERM_PROGRAM.
        for value in &["ghostty", "Ghostty", "ghostty.app"] {
            let terminal = detect_terminal_from_env(Some(value), None, None)
                .unwrap_or_else(|_| panic!("TERM_PROGRAM={value:?} should be Ghostty"));
            assert!(format!("{terminal:?}").contains("Ghostty"));
        }
    }

    #[test]
    fn test_detect_terminal_wezterm() {
        // Act — WezTerm sets TERM_PROGRAM=WezTerm.
        let terminal = detect_terminal_from_env(Some("WezTerm"), None, None)
            .expect("WezTerm should be detected");

        // Assert
        assert!(format!("{terminal:?}").contains("WezTerm"));
    }

    #[test]
    fn test_detect_terminal_kitty_via_term() {
        // Act — kitty does not set TERM_PROGRAM; it sets TERM=xterm-kitty.
        let terminal = detect_terminal_from_env(None, Some("xterm-kitty"), None)
            .expect("kitty should be detected via TERM");

        // Assert
        assert!(format!("{terminal:?}").contains("Kitty"));
    }

    #[test]
    fn test_detect_terminal_kitty_via_window_id() {
        // Act — kitty also sets KITTY_WINDOW_ID.
        let terminal = detect_terminal_from_env(None, None, Some("1"))
            .expect("kitty should be detected via KITTY_WINDOW_ID");

        // Assert
        assert!(format!("{terminal:?}").contains("Kitty"));
    }

    #[test]
    fn test_detect_terminal_unsupported_terminal_returns_error() {
        // Arrange
        let term_program = Some("gnome-terminal");

        // Act
        let result = detect_terminal_from_env(term_program, None, None);

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
        let result = detect_terminal_from_env(None, None, None);

        // Assert
        let err = result.expect_err("missing terminal env should yield an error");
        assert!(
            matches!(err, TerminalError::TerminalDetectionFailed(_)),
            "expected TerminalDetectionFailed, got {err:?}"
        );
    }
}
