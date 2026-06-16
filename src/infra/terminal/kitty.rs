//! kitty terminal implementation using its remote-control CLI (`kitty @`).

use std::path::Path;

use super::command::{login_shell, run_command};
use super::{Terminal, TerminalError};
use crate::infra::agent::Agent;

/// Concrete implementation of [`Terminal`] for kitty.
///
/// Relies on kitty's remote control (`kitty @`), which requires
/// `allow_remote_control` to be enabled. The current kitty instance is targeted
/// automatically via the `KITTY_LISTEN_ON` environment variable.
#[derive(Debug)]
pub struct Kitty;

impl Terminal for Kitty {
    /// Opens a new kitty tab and launches the agent.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::TabOpenFailed`] if `kitty @ launch` fails (e.g.
    /// remote control is disabled).
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let shell = login_shell();
        let args = build_launch_args(
            "tab",
            None,
            &worktree_path.display().to_string(),
            agent.command_name(),
            &shell,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("kitty", &arg_refs).map_err(TerminalError::TabOpenFailed)
    }

    /// Splits the current kitty window vertically (side by side) and runs `command`.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::PaneSplitFailed`] if `kitty @ launch` fails.
    fn split_pane(&self, working_dir: &Path, command: &str) -> Result<(), TerminalError> {
        let shell = login_shell();
        let args = build_launch_args(
            "window",
            Some("vsplit"),
            &working_dir.display().to_string(),
            command,
            &shell,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("kitty", &arg_refs).map_err(TerminalError::PaneSplitFailed)
    }
}

/// Builds the `kitty @ launch` argument vector.
///
/// `launch_type` is kitty's `--type` (`tab` or `window`); `location` maps to
/// `--location` (e.g. `vsplit` for a side-by-side split). The command runs inside a
/// login shell that re-execs after `command` exits so the tab/pane stays open.
fn build_launch_args(
    launch_type: &str,
    location: Option<&str>,
    cwd: &str,
    command: &str,
    shell: &str,
) -> Vec<String> {
    let mut args = vec![
        "@".to_string(),
        "launch".to_string(),
        format!("--type={}", launch_type),
    ];
    if let Some(loc) = location {
        args.push(format!("--location={}", loc));
    }
    args.push("--cwd".to_string());
    args.push(cwd.to_string());
    args.push("--".to_string());
    args.push(shell.to_string());
    args.push("-lc".to_string());
    args.push(format!("{}; exec {}", command, shell));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_launch_args_tab_contains_type_cwd_and_command() {
        // Act
        let args = build_launch_args("tab", None, "/home/user/ws", "claude", "/bin/zsh");

        // Assert
        assert!(args.contains(&"--type=tab".to_string()));
        assert!(args.contains(&"/home/user/ws".to_string()));
        assert!(args.iter().any(|a| a.contains("claude")));
        // No split location for a plain tab.
        assert!(!args.iter().any(|a| a.starts_with("--location")));
    }

    #[test]
    fn test_build_launch_args_split_uses_vsplit_location() {
        // Act
        let args = build_launch_args("window", Some("vsplit"), "/ws", "silo launch", "/bin/sh");

        // Assert — side-by-side split matching iTerm2's "split vertically".
        assert!(args.contains(&"--type=window".to_string()));
        assert!(args.contains(&"--location=vsplit".to_string()));
    }

    #[test]
    fn test_build_launch_args_reexecs_shell_to_keep_pane_open() {
        // Act
        let args = build_launch_args("tab", None, "/ws", "claude", "/bin/zsh");

        // Assert — the inner shell command re-execs the shell after the agent exits.
        assert!(args.iter().any(|a| a == "claude; exec /bin/zsh"));
    }
}
