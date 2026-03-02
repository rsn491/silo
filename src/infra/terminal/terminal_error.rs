use std::fmt;

#[derive(Debug)]
pub enum TerminalError {
    TerminalNotSupported(String),
    TerminalDetectionFailed(String),
    TabOpenFailed(String),
    PaneSplitFailed(String),
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalError::TerminalNotSupported(msg) => {
                write!(f, "unsupported terminal: {}", msg)
            }
            TerminalError::TerminalDetectionFailed(msg) => {
                write!(f, "terminal detection failed: {}", msg)
            }
            TerminalError::TabOpenFailed(msg) => {
                write!(f, "failed to open tab: {}", msg)
            }
            TerminalError::PaneSplitFailed(msg) => {
                write!(f, "failed to split pane: {}", msg)
            }
        }
    }
}

impl std::error::Error for TerminalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
