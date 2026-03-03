use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("unsupported terminal: {0}")]
    TerminalNotSupported(String),
    #[error("terminal detection failed: {0}")]
    TerminalDetectionFailed(String),
    #[error("failed to open tab: {0}")]
    TabOpenFailed(String),
    #[error("failed to split pane: {0}")]
    PaneSplitFailed(String),
}
