use std::path::Path;

use crate::infra::agent::Agent;
use crate::infra::osascript::run_osascript;
use super::{Terminal, TerminalError};

#[derive(Debug)]
pub struct ITerm2;

impl Terminal for ITerm2 {
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let path_str = worktree_path.display().to_string();
        let escaped_path = path_str.replace('\'', "'\\''");
        let cmd = agent.command();
        let program = cmd.get_program().to_string_lossy().to_string();
        let escaped_program = program.replace('\'', "'\\''");
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

    fn split_pane(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let path_str = worktree_path.display().to_string();
        let escaped_path = path_str.replace('\'', "'\\''");
        let cmd = agent.command();
        let program = cmd.get_program().to_string_lossy().to_string();
        let escaped_program = program.replace('\'', "'\\''");
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
