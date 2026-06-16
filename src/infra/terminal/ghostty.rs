//! Ghostty terminal implementation.
//!
//! Ghostty exposes no AppleScript dictionary and no remote-control CLI, so the only
//! clean, robust way to launch an agent is to open a new window via the `open` CLI.
//! `--tab` therefore opens a new Ghostty window; `--split-pane` is not supported.

use std::path::Path;

use super::command::{login_shell, run_command};
use super::{Terminal, TerminalError};
use crate::infra::agent::Agent;

/// Concrete implementation of [`Terminal`] for Ghostty.
#[derive(Debug)]
pub struct Ghostty;

impl Terminal for Ghostty {
    /// Opens a new Ghostty window running the agent in `worktree_path`.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::TabOpenFailed`] if the `open` command fails.
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let shell = login_shell();
        let args = build_open_window_args(
            &worktree_path.display().to_string(),
            agent.command_name(),
            &shell,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("open", &arg_refs).map_err(TerminalError::TabOpenFailed)
    }

    /// Ghostty has no programmatic split-pane support, so this is unsupported.
    ///
    /// # Errors
    ///
    /// Always returns [`TerminalError::PaneSplitFailed`].
    fn split_pane(&self, _working_dir: &Path, _command: &str) -> Result<(), TerminalError> {
        Err(TerminalError::PaneSplitFailed(
            "Ghostty does not support splitting panes programmatically; \
             use --tab to open a new window instead"
                .to_string(),
        ))
    }
}

/// Builds the `open -na Ghostty --args …` argument vector for a new window.
///
/// Ghostty runs the command via `-e`; the command is a login shell that re-execs
/// after the agent exits so the window stays open.
fn build_open_window_args(cwd: &str, command: &str, shell: &str) -> Vec<String> {
    vec![
        "-na".to_string(),
        "Ghostty".to_string(),
        "--args".to_string(),
        format!("--working-directory={}", cwd),
        "-e".to_string(),
        shell.to_string(),
        "-lc".to_string(),
        format!("{}; exec {}", command, shell),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_open_window_args_contains_working_directory_and_command() {
        // Act
        let args = build_open_window_args("/home/user/ws", "claude", "/bin/zsh");

        // Assert
        assert!(args.contains(&"Ghostty".to_string()));
        assert!(args.contains(&"--working-directory=/home/user/ws".to_string()));
        assert!(args.iter().any(|a| a == "claude; exec /bin/zsh"));
    }

    #[test]
    fn test_split_pane_returns_unsupported_error() {
        // Act
        let result = Ghostty.split_pane(Path::new("/ws"), "silo launch");

        // Assert
        let err = result.expect_err("Ghostty split_pane should be unsupported");
        assert!(matches!(err, TerminalError::PaneSplitFailed(_)));
    }
}
