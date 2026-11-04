//! Utilities for running AppleScript commands on macOS.

use std::process::Command;

/// Runs an AppleScript command using `osascript`.
///
/// # Errors
///
/// Returns an error if the `osascript` command fails to execute or returns a non-zero exit status.
pub fn run_osascript(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| format!("failed to run osascript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript failed: {}", stderr));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn reports_missing_osascript() {
        // Given
        // (no setup needed)

        // When
        let err = run_osascript("return 1").unwrap_err();

        // Then
        assert!(err.starts_with("failed to run osascript"));
    }
}
