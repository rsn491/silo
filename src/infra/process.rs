use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
pub enum ProcessError {
    CommandFailed(String),
    ParseError(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            ProcessError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for ProcessError {}

pub trait ProcessOperations {
    fn find_processes_by_name(&self, name: &str) -> Result<Vec<(u32, String)>, ProcessError>;
    fn get_process_cwd(&self, pid: u32) -> Result<PathBuf, ProcessError>;
}

#[derive(Default)]
pub struct SystemProcess;

impl ProcessOperations for SystemProcess {
    fn find_processes_by_name(&self, name: &str) -> Result<Vec<(u32, String)>, ProcessError> {
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
        Ok(parse_ps_output(&stdout, name, own_pid))
    }

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

fn parse_ps_output(output: &str, pattern: &str, own_pid: u32) -> Vec<(u32, String)> {
    let mut processes = Vec::new();

    for line in output.lines().skip(1) {
        // Skip header line
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split into PID and args
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            continue;
        }

        let pid = match parts[0].trim().parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Skip own process
        if pid == own_pid {
            continue;
        }

        let args = parts[1].trim();

        // Check if args contains the pattern
        if args.contains(pattern) {
            processes.push((pid, args.to_string()));
        }
    }

    processes
}

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
        let output = "  PID ARGS
  123 /usr/bin/claude --some-args
  456 /usr/bin/other-process
  789 /path/to/claude code";

        let processes = parse_ps_output(output, "claude", 999);
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].0, 123);
        assert!(processes[0].1.contains("claude"));
        assert_eq!(processes[1].0, 789);
        assert!(processes[1].1.contains("claude"));
    }

    #[test]
    fn test_parse_ps_output_excludes_own_pid() {
        let output = "  PID ARGS
  123 /usr/bin/claude --some-args
  456 /usr/bin/claude --other-args";

        let processes = parse_ps_output(output, "claude", 123);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].0, 456);
    }

    #[test]
    fn test_parse_ps_output_no_matches() {
        let output = "  PID ARGS
  123 /usr/bin/other
  456 /usr/bin/different";

        let processes = parse_ps_output(output, "claude", 999);
        assert_eq!(processes.len(), 0);
    }

    #[test]
    fn test_parse_lsof_cwd_output_normal() {
        let output = "p123
n/path/to/worktree";

        let cwd = parse_lsof_cwd_output(output).unwrap();
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
