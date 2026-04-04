//! Shared utilities for workspace implementations.

use std::path::PathBuf;

use uuid::Uuid;

use super::agent_launcher::LaunchError;
use super::git_branch_service::UUID_SUFFIX_LEN;
use super::silo_config::SiloConfig;
use crate::infra::git::GitOperations;

/// Generates a unique workspace path within the Silo directory.
///
/// The path is `<silo_dir>/<project_name>-<8-char UUID>`.
///
/// # Errors
///
/// Returns [`LaunchError`] if the Silo directory cannot be determined or if the
/// project name cannot be resolved.
pub fn generate_workspace_path<G: GitOperations>(
    git: &G,
    missing_silo_dir_error: &str,
) -> Result<PathBuf, LaunchError> {
    let base_dir = SiloConfig::get_silo_dir()
        .ok_or_else(|| LaunchError::AgentSpawnError(missing_silo_dir_error.into()))?;
    let name = format!(
        "{}-{}",
        git.get_project_name()?,
        &Uuid::new_v4().to_string()[..UUID_SUFFIX_LEN]
    );
    Ok(base_dir.join(&name))
}

/// Parses porcelain Git status output to determine uncommitted changes.
///
/// Returns `(has_changes, file_count)` where `file_count` is the number of
/// non-empty lines in the status output.
pub fn parse_uncommitted_changes(status_output: &str) -> (bool, usize) {
    let file_count = status_output
        .lines()
        .filter(|line| !line.is_empty())
        .count();
    (file_count > 0, file_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uncommitted_changes_with_files() {
        let status_output = " M file1.txt\n M file2.txt\n M file3.txt\n";
        let (has_changes, count) = parse_uncommitted_changes(status_output);
        assert!(has_changes);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_parse_uncommitted_changes_empty() {
        let status_output = "";
        let (has_changes, count) = parse_uncommitted_changes(status_output);
        assert!(!has_changes);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_parse_uncommitted_changes_with_empty_lines() {
        let status_output = " M file1.txt\n\n M file2.txt\n";
        let (has_changes, count) = parse_uncommitted_changes(status_output);
        assert!(has_changes);
        assert_eq!(count, 2); // Should only count non-empty lines.
    }
}
