//! WezTerm terminal implementation using its remote-control CLI (`wezterm cli`).

use std::path::Path;

use super::command::{login_shell, run_command};
use super::{Terminal, TerminalError};
use crate::infra::agent::Agent;

/// Concrete implementation of [`Terminal`] for WezTerm.
///
/// Uses `wezterm cli`, which targets the current pane automatically via the
/// `WEZTERM_PANE` environment variable.
#[derive(Debug)]
pub struct WezTerm;

impl Terminal for WezTerm {
    /// Opens a new WezTerm tab and launches the agent.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::TabOpenFailed`] if `wezterm cli spawn` fails.
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), TerminalError> {
        let shell = login_shell();
        let args = build_spawn_args(
            &worktree_path.display().to_string(),
            agent.command_name(),
            &shell,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("wezterm", &arg_refs).map_err(TerminalError::TabOpenFailed)
    }

    /// Splits the current WezTerm pane to the right (side by side) and runs `command`.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalError::PaneSplitFailed`] if `wezterm cli split-pane` fails.
    fn split_pane(&self, working_dir: &Path, command: &str) -> Result<(), TerminalError> {
        let shell = login_shell();
        let args = build_split_args(&working_dir.display().to_string(), command, &shell);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("wezterm", &arg_refs).map_err(TerminalError::PaneSplitFailed)
    }
}

/// Builds the `wezterm cli spawn` argument vector for a new tab.
fn build_spawn_args(cwd: &str, command: &str, shell: &str) -> Vec<String> {
    let mut args = vec![
        "cli".to_string(),
        "spawn".to_string(),
        "--cwd".to_string(),
        cwd.to_string(),
        "--".to_string(),
    ];
    args.extend(shell_invocation(command, shell));
    args
}

/// Builds the `wezterm cli split-pane` argument vector for a side-by-side split.
fn build_split_args(cwd: &str, command: &str, shell: &str) -> Vec<String> {
    let mut args = vec![
        "cli".to_string(),
        "split-pane".to_string(),
        // --right gives a side-by-side split, matching iTerm2's "split vertically".
        "--right".to_string(),
        "--cwd".to_string(),
        cwd.to_string(),
        "--".to_string(),
    ];
    args.extend(shell_invocation(command, shell));
    args
}

/// Builds the trailing `<shell> -lc "<command>; exec <shell>"` invocation that keeps
/// the tab/pane open after the agent exits.
fn shell_invocation(command: &str, shell: &str) -> Vec<String> {
    vec![
        shell.to_string(),
        "-lc".to_string(),
        format!("{}; exec {}", command, shell),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_spawn_args_contains_cwd_and_command() {
        // Act
        let args = build_spawn_args("/home/user/ws", "claude", "/bin/zsh");

        // Assert
        assert!(args.contains(&"spawn".to_string()));
        assert!(args.contains(&"/home/user/ws".to_string()));
        assert!(args.iter().any(|a| a == "claude; exec /bin/zsh"));
    }

    #[test]
    fn test_build_split_args_uses_right_for_side_by_side() {
        // Act
        let args = build_split_args("/ws", "silo launch", "/bin/sh");

        // Assert
        assert!(args.contains(&"split-pane".to_string()));
        assert!(args.contains(&"--right".to_string()));
        assert!(args.iter().any(|a| a == "silo launch; exec /bin/sh"));
    }
}
