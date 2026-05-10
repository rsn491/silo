//! Traits and implementations for interacting with system processes.

use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

/// Represents errors that can occur during process operations.
#[derive(Debug, Error)]
pub enum ProcessError {
    /// A system command (e.g., `ps`, `lsof`) failed to execute or returned an error.
    #[error("Command failed: {0}")]
    CommandFailed(String),
    /// Failed to parse the output of a system command.
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Trait defining operations for inspecting and finding system processes.
#[cfg_attr(test, mockall::automock)]
pub trait ProcessOperations {
    /// Finds processes whose command-line arguments match any of the provided names.
    ///
    /// # Errors
    ///
    /// Returns a [`ProcessError`] if the underlying system command fails.
    #[cfg_attr(test, mockall::concretize)]
    fn find_processes_by_names(&self, names: &[&str]) -> Result<Vec<(u32, String)>, ProcessError>;

    /// Gets the current working directory of a process by its PID.
    ///
    /// # Errors
    ///
    /// Returns a [`ProcessError`] if `lsof` fails or its output cannot be parsed.
    fn get_process_cwd(&self, pid: u32) -> Result<PathBuf, ProcessError>;
}

/// A concrete implementation of [`ProcessOperations`] using standard system tools.
#[derive(Default, Clone)]
pub struct SystemProcess;

impl ProcessOperations for SystemProcess {
    /// Finds processes using the `ps` command.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::CommandFailed`] if the `ps` command fails.
    fn find_processes_by_names(&self, names: &[&str]) -> Result<Vec<(u32, String)>, ProcessError> {
        let output = Command::new("ps")
            .args(["-eo", "pid,args"])
            .output()
            .map_err(|e| ProcessError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProcessError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let own_pid = std::process::id();
        Ok(parse_ps_output_with_patterns(&stdout, names, own_pid))
    }

    /// Gets the process CWD using the `lsof` command.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::CommandFailed`] if the `lsof` command fails,
    /// or [`ProcessError::ParseError`] if the output format is unexpected.
    fn get_process_cwd(&self, pid: u32) -> Result<PathBuf, ProcessError> {
        let output = Command::new("lsof")
            .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
            .output()
            .map_err(|e| ProcessError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProcessError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_lsof_cwd_output(&stdout)
    }
}

/// Returns `true` if the process with `pid` is currently running.
///
/// On Linux this is done by checking for the existence of `/proc/<pid>`,
/// which is reliable and requires no external crates. On other platforms
/// the check falls back to a conservative `true` (assume alive) so that
/// a live lock is never incorrectly reclaimed on an unsupported platform.
pub fn pid_is_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        true
    }
}

/// Parses the output of `ps -eo pid,args` and filters by the provided patterns.
fn parse_ps_output_with_patterns(
    output: &str,
    patterns: &[&str],
    own_pid: u32,
) -> Vec<(u32, String)> {
    let mut processes = Vec::new();

    for line in output.lines().skip(1) {
        // Skip header line.
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split into PID and args.
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            continue;
        }

        let pid = match parts[0].trim().parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Skip own process.
        if pid == own_pid {
            continue;
        }

        let args = parts[1].trim();

        // Check if args contains any of the patterns.
        if patterns.iter().any(|pattern| args.contains(pattern)) {
            processes.push((pid, args.to_string()));
        }
    }

    processes
}

/// Parses the output of `lsof -Fn` to extract the CWD path.
///
/// # Errors
///
/// Returns [`ProcessError::ParseError`] if no line starts with 'n'.
fn parse_lsof_cwd_output(output: &str) -> Result<PathBuf, ProcessError> {
    for line in output.lines() {
        if let Some(path) = line.strip_prefix('n') {
            return Ok(PathBuf::from(path));
        }
    }

    Err(ProcessError::ParseError(
        "No cwd path found in lsof output".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ps_output_matching_processes() {
        // Arrange
        let output = "  PID ARGS
  123 /usr/bin/claude --some-args
  456 /usr/bin/other-process
  789 /path/to/claude code";

        // Act
        let processes = parse_ps_output_with_patterns(output, &["claude"], 999);

        // Assert
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].0, 123);
        assert!(processes[0].1.contains("claude"));
        assert_eq!(processes[1].0, 789);
        assert!(processes[1].1.contains("claude"));
    }

    #[test]
    fn test_parse_ps_output_excludes_own_pid() {
        // Arrange
        let output = "  PID ARGS
  123 /usr/bin/claude --some-args
  456 /usr/bin/claude --other-args";

        // Act
        let processes = parse_ps_output_with_patterns(output, &["claude"], 123);

        // Assert
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].0, 456);
    }

    #[test]
    fn test_parse_ps_output_no_matches() {
        // Arrange
        let output = "  PID ARGS
  123 /usr/bin/other
  456 /usr/bin/different";

        // Act
        let processes = parse_ps_output_with_patterns(output, &["claude"], 999);

        // Assert
        assert_eq!(processes.len(), 0);
    }

    #[test]
    fn test_parse_ps_output_with_multiple_patterns() {
        // Arrange
        let output = "  PID ARGS
  123 /usr/bin/claude --some-args
  456 /usr/bin/opencode --other-args
  789 /usr/bin/other-process
  101 /path/to/claude code";

        // Act
        let processes = parse_ps_output_with_patterns(output, &["claude", "opencode"], 999);

        // Assert
        assert_eq!(processes.len(), 3);
        assert_eq!(processes[0].0, 123);
        assert!(processes[0].1.contains("claude"));
        assert_eq!(processes[1].0, 456);
        assert!(processes[1].1.contains("opencode"));
        assert_eq!(processes[2].0, 101);
        assert!(processes[2].1.contains("claude"));
    }

    #[test]
    fn test_parse_lsof_cwd_output_normal() {
        // Arrange
        let output = "p123
n/path/to/worktree";

        // Act
        let cwd = parse_lsof_cwd_output(output).expect("should parse valid lsof output");

        // Assert
        assert_eq!(cwd, PathBuf::from("/path/to/worktree"));
    }

    #[test]
    fn test_parse_lsof_cwd_output_missing_n_line() {
        let output = "p123
cDIR";

        let result = parse_lsof_cwd_output(output);
        assert!(result.is_err());
    }
}
