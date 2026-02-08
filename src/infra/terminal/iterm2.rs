use std::path::Path;

use crate::infra::osascript::run_osascript;
use crate::services::agent_launcher::LaunchError;

use super::Terminal;

#[derive(Debug)]
pub struct ITerm2;

impl Terminal for ITerm2 {
    fn open_tab(&self, worktree_path: &Path) -> Result<(), LaunchError> {
        let path_str = worktree_path.display().to_string();
        let script = format!(
            r#"tell application "iTerm2"
                activate
                if (count of windows) is 0 then
                    create window with default profile
                end if
                tell current window
                    create tab with default profile
                    tell current session
                        write text "cd '{}' && claude"
                    end tell
                end tell
            end tell"#,
            path_str
        );
        run_osascript(&script).map_err(LaunchError::AgentSpawnError)
    }

    fn split_pane(&self, worktree_path: &Path) -> Result<(), LaunchError> {
        let path_str = worktree_path.display().to_string();
        let script = format!(
            r#"tell application "iTerm2"
                activate
                if (count of windows) is 0 then
                    create window with default profile
                end if
                tell current session of current window
                    set newSession to (split vertically with default profile)
                    tell newSession
                        write text "cd '{}' && claude"
                    end tell
                end tell
            end tell"#,
            path_str
        );
        run_osascript(&script).map_err(LaunchError::AgentSpawnError)
    }
}
