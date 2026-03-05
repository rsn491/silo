//! iTerm2 terminal implementation using AppleScript.

use std::path::Path;

use super::{Terminal, TerminalError};
use crate::infra::agent::Agent;
use crate::infra::osascript::run_osascript;

/// Concrete implementation of [`Terminal`] for iTerm2.
#[derive(Debug)]
pub struct ITerm2;

impl Terminal for ITerm2 {
    /// Opens a new tab in iTerm2 and launches the agent.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::TabOpenFailed`] if the AppleScript fails to execute.
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let path_str = worktree_path.display().to_string();
        let escaped_path = path_str.replace('\'', "'\\''");
        let command_name = agent.command_name();
        let escaped_program = command_name.replace('\'', "'\\''");
        let script = format!(
            r#"tell application "iTerm2"
                activate
                if (count of windows) is 0 then
                    create window with default profile
                end if
                tell current window
                    create tab with default profile
                    tell current session
                        write text "cd '{}' && {}"
                    end tell
                end tell
            end tell"#,
            escaped_path, escaped_program
        );
        run_osascript(&script).map_err(|e| TerminalError::TabOpenFailed(e.to_string()))
    }

    /// Splits the current pane in iTerm2 vertically and launches the agent.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::PaneSplitFailed`] if the AppleScript fails to execute.
    fn split_pane(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let path_str = worktree_path.display().to_string();
        let escaped_path = path_str.replace('\'', "'\\''");
        let command_name = agent.command_name();
        let escaped_program = command_name.replace('\'', "'\\''");
        let script = format!(
            r#"tell application "iTerm2"
                activate
                if (count of windows) is 0 then
                    create window with default profile
                end if
                tell current session of current window
                    set newSession to (split vertically with default profile)
                    tell newSession
                        write text "cd '{}' && {}"
                    end tell
                end tell
            end tell"#,
            escaped_path, escaped_program
        );
        run_osascript(&script).map_err(|e| TerminalError::PaneSplitFailed(e.to_string()))
    }
}
