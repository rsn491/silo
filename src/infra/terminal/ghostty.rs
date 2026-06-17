//! Ghostty terminal implementation.
//!
//! Ghostty exposes no scripting API for splitting panes, but since v1.3.0 it ships a
//! `ghostty +new-window` CLI action that opens a new window in the running instance.
//! This works on both macOS and Linux and assumes `ghostty` is on `$PATH`.
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
    /// Returns [`TerminalError::TabOpenFailed`] if the `ghostty` command fails.
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let shell = login_shell();
        let args = build_new_window_args(
            &worktree_path.display().to_string(),
            agent.command_name(),
            &shell,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("ghostty", &arg_refs).map_err(TerminalError::TabOpenFailed)
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

/// Builds the `ghostty +new-window …` argument vector for a new window.
///
/// `--working-directory` is always passed explicitly: omitting it makes Ghostty
/// auto-append the cwd, which corrupts the `-e` command on v1.3.0. Everything after
/// `-e` is the command to run — a login shell that re-execs after the agent exits so
/// the window stays open.
fn build_new_window_args(cwd: &str, command: &str, shell: &str) -> Vec<String> {
    vec![
        "+new-window".to_string(),
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
    fn test_build_new_window_args_contains_working_directory_and_command() {
        // Act
        let args = build_new_window_args("/home/user/ws", "claude", "/bin/zsh");

        // Assert
        assert!(args.contains(&"+new-window".to_string()));
        assert!(args.contains(&"--working-directory=/home/user/ws".to_string()));
        assert!(args.iter().any(|a| a == "claude; exec /bin/zsh"));
    }

    #[test]
    fn test_build_new_window_args_passes_working_directory_before_command() {
        // Assert — --working-directory must precede -e, otherwise Ghostty's cwd
        // auto-detection corrupts the -e command (v1.3.0 bug).
        let args = build_new_window_args("/ws", "claude", "/bin/zsh");
        let wd = args
            .iter()
            .position(|a| a.starts_with("--working-directory"))
            .expect("working-directory arg present");
        let e = args.iter().position(|a| a == "-e").expect("-e arg present");
        assert!(wd < e, "--working-directory must come before -e");
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
