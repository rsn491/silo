//! Error types for terminal operations.

use thiserror::Error;

/// Represents errors that can occur during terminal interactions.
#[derive(Debug, Error)]
pub enum TerminalError {
    /// The detected terminal is not supported by Silo.
    #[error("unsupported terminal: {0}")]
    TerminalNotSupported(String),
    /// Failed to determine which terminal is being used.
    #[error("terminal detection failed: {0}")]
    TerminalDetectionFailed(String),
    /// Failed to open a new tab in the terminal.
    #[error("failed to open tab: {0}")]
    TabOpenFailed(String),
    /// Failed to split the current pane in the terminal.
    #[error("failed to split pane: {0}")]
    PaneSplitFailed(String),
}
