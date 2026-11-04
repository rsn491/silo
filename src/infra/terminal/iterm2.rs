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
        let script = build_open_tab_script(&path_str, agent.command_name());
        run_osascript(&script).map_err(|e| TerminalError::TabOpenFailed(e.to_string()))
    }

    /// Splits the current pane in iTerm2 vertically and runs `command` from `working_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::PaneSplitFailed`] if the AppleScript fails to execute.
    fn split_pane(&self, working_dir: &Path, command: &str) -> Result<(), TerminalError> {
        let path_str = working_dir.display().to_string();
        let script = build_split_pane_script(&path_str, command);
        run_osascript(&script).map_err(|e| TerminalError::PaneSplitFailed(e.to_string()))
    }
}

/// Builds the AppleScript for opening a new iTerm2 tab and launching an agent.
///
/// Single quotes in `path_str` and `agent_name` are escaped for use inside a shell
/// single-quoted string (`'\''`).
fn build_open_tab_script(path_str: &str, agent_name: &str) -> String {
    let escaped_path = path_str.replace('\'', "'\\''");
    let escaped_program = agent_name.replace('\'', "'\\''");
    format!(
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
    )
}

/// Builds the AppleScript for splitting the current iTerm2 pane and running a command.
///
/// Single quotes in `path_str` are escaped; double quotes in `command` are backslash-escaped.
fn build_split_pane_script(path_str: &str, command: &str) -> String {
    let escaped_path = path_str.replace('\'', "'\\''");
    let escaped_command = command.replace('"', "\\\"");
    format!(
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
        escaped_path, escaped_command
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_open_tab_and_split_pane_fail_without_osascript() {
        // Given
        let path = Path::new("/tmp");

        // When
        let actual_1 = matches!(
            ITerm2.open_tab(path, &Agent::ClaudeCode),
            Err(TerminalError::TabOpenFailed(_))
        );
        let actual_2 = matches!(
            ITerm2.split_pane(path, "silo launch"),
            Err(TerminalError::PaneSplitFailed(_))
        );

        // Then
        assert!(actual_1);
        assert!(actual_2);
    }

    #[test]
    fn test_build_open_tab_script_contains_path_and_agent() {
        // Given
        let path = "/home/user/workspace";
        let agent = "claude";

        // When
        let script = build_open_tab_script(path, agent);

        // Then
        assert!(script.contains("cd '/home/user/workspace'"));
        assert!(script.contains("&& claude"));
    }

    #[test]
    fn test_build_open_tab_script_escapes_single_quote_in_path() {
        // Given
        let path = "/path/with'quote/repo";

        // When
        let script = build_open_tab_script(path, "claude");

        // Then — single quotes in paths use POSIX '\'' escaping
        assert!(script.contains("cd '/path/with'\\''quote/repo'"));
    }

    #[test]
    fn test_build_open_tab_script_escapes_single_quote_in_agent_name() {
        // Given
        let agent = "my'agent";

        // When
        let script = build_open_tab_script("/workspace", agent);

        // Then
        assert!(script.contains("&& my'\\''agent"));
    }

    #[test]
    fn test_build_split_pane_script_contains_path_and_command() {
        // Given
        let path = "/home/user/workspace";
        let command = "silo launch";

        // When
        let script = build_split_pane_script(path, command);

        // Then
        assert!(script.contains("cd '/home/user/workspace'"));
        assert!(script.contains("&& silo launch"));
    }

    #[test]
    fn test_build_split_pane_script_escapes_double_quotes_in_command() {
        // Given
        let command = r#"echo "hello""#;

        // When
        let script = build_split_pane_script("/workspace", command);

        // Then — double quotes are backslash-escaped so they survive the outer double-quoted string
        assert!(script.contains(r#"echo \"hello\""#));
    }

    #[test]
    fn test_build_split_pane_script_escapes_single_quote_in_path() {
        // Given
        let path = "/path/it's/here";

        // When
        let script = build_split_pane_script(path, "silo launch");

        // Then
        assert!(script.contains("cd '/path/it'\\''s/here'"));
    }
}
