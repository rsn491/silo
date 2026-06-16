//! Shared helper for running CLI commands used by terminal implementations.

use std::process::Command;

/// Runs `program` with `args`, returning an error if it fails to launch or exits
/// with a non-zero status.
///
/// Used by the CLI-driven terminal backends (kitty, WezTerm, Ghostty) that control
/// the terminal via its own command-line interface rather than AppleScript.
///
/// # Errors
///
/// Returns an error string if the process cannot be spawned or returns a non-zero
/// exit status (with stderr included for diagnostics).
pub fn run_command(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {}: {}", program, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{} failed: {}", program, stderr));
    }
    Ok(())
}

/// Returns the user's login shell from `$SHELL`, falling back to `/bin/sh`.
///
/// Used to wrap agent commands so the new tab/pane keeps an interactive shell open
/// after the agent exits.
pub fn login_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}
