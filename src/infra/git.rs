use std::path::{Path, PathBuf};
use std::process::Command;

use crate::infra::git_error::GitError;

pub trait GitOperations {
    fn get_repo_root(&self) -> Result<PathBuf, GitError>;
    fn get_project_name(&self) -> Result<String, GitError>;
    fn create_worktree(&self, path: &Path, branch: &str) -> Result<(), GitError>;
}

#[derive(Default)]
pub struct Git;

impl GitOperations for Git {
    fn get_repo_root(&self) -> Result<PathBuf, GitError> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(GitError::NotAGitRepo);
        }

        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(path))
    }

    fn get_project_name(&self) -> Result<String, GitError> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(name) = extract_project_name_from_url(&url) {
                return Ok(name);
            }
        }

        self.get_repo_root()?
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GitError::CommandFailed("could not determine project name".to_string()))
    }

    fn create_worktree(&self, path: &Path, branch: &str) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["worktree", "add", "-b", branch])
            .arg(path)
            .output()
            .map_err(|e| GitError::WorktreeCreationFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::WorktreeCreationFailed(stderr.to_string()));
        }

        Ok(())
    }
}

fn extract_project_name_from_url(url: &str) -> Option<String> {
    // Handle SSH format: git@github.com:user/repo.git
    // Handle HTTPS format: https://github.com/user/repo.git
    let name = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .or_else(|| url.rsplit(':').next())?;

    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_project_name_https() {
        assert_eq!(
            extract_project_name_from_url("https://github.com/user/myrepo.git"),
            Some("myrepo".to_string())
        );
    }

    #[test]
    fn test_extract_project_name_ssh() {
        assert_eq!(
            extract_project_name_from_url("git@github.com:user/myrepo.git"),
            Some("myrepo".to_string())
        );
    }

    #[test]
    fn test_extract_project_name_no_git_suffix() {
        assert_eq!(
            extract_project_name_from_url("https://github.com/user/myrepo"),
            Some("myrepo".to_string())
        );
    }
}
