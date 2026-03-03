use std::path::{Path, PathBuf};
use std::process::Command;

use crate::infra::git_error::GitError;
use crate::infra::workspace_kind::WorkspaceKind;

#[derive(Debug, Clone, Default)]
pub struct GitWorkspaceInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub kind: WorkspaceKind,
    pub has_uncommitted_changes: bool,
    pub uncommitted_file_count: usize,
    pub commits_ahead: usize,
    pub commits_behind: usize,
}

#[cfg_attr(test, mockall::automock)]
pub trait GitOperations {
    fn get_repo_root(&self) -> Result<PathBuf, GitError>;
    fn get_project_name(&self) -> Result<String, GitError>;
    fn create_worktree(&self, path: &Path, branch: &str) -> Result<(), GitError>;
    fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError>;
    fn remove_worktree(&self, path: &Path) -> Result<(), GitError>;
    fn get_default_remote_branch(&self) -> Result<String, GitError>;
    fn get_status_porcelain(&self, worktree_path: &Path) -> Result<String, GitError>;
    fn count_commits_ahead(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError>;
    fn count_commits_behind(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError>;
    fn clone_local(&self, source: &Path, dest: &Path) -> Result<(), GitError>;
    fn checkout_new_branch(&self, path: &Path, branch: &str) -> Result<(), GitError>;
    fn get_current_branch(&self, path: &Path) -> Result<Option<String>, GitError>;
}

#[derive(Default, Clone)]
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

    fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_worktree_list(&stdout))
    }

    fn remove_worktree(&self, path: &Path) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(path)
            .output()
            .map_err(|e| GitError::WorktreeRemovalFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::WorktreeRemovalFailed(stderr.to_string()));
        }

        Ok(())
    }

    fn get_default_remote_branch(&self) -> Result<String, GitError> {
        let output = Command::new("git")
            .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout)
                .trim()
                .strip_prefix("refs/remotes/")
                .unwrap_or("origin/main")
                .to_string();
            Ok(branch)
        } else {
            // Fallback to origin/main if command fails
            Ok("origin/main".to_string())
        }
    }

    fn get_status_porcelain(&self, worktree_path: &Path) -> Result<String, GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(worktree_path)
            .args(["status", "--porcelain"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn count_commits_ahead(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(worktree_path)
            .args(["rev-list", "--count"])
            .arg(format!("{}..HEAD", base_branch))
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let count = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<usize>()
                .unwrap_or(0);
            Ok(count)
        } else {
            Ok(0)
        }
    }

    fn count_commits_behind(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(worktree_path)
            .args(["rev-list", "--count"])
            .arg(format!("HEAD..{}", base_branch))
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let count = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<usize>()
                .unwrap_or(0);
            Ok(count)
        } else {
            Ok(0)
        }
    }

    fn clone_local(&self, source: &Path, dest: &Path) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["clone", "--local"])
            .arg(source)
            .arg(dest)
            .output()
            .map_err(|e| GitError::CloneFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CloneFailed(stderr.to_string()));
        }

        Ok(())
    }

    fn checkout_new_branch(&self, path: &Path, branch: &str) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["checkout", "-b", branch])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        Ok(())
    }

    fn get_current_branch(&self, path: &Path) -> Result<Option<String>, GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch == "HEAD" {
            Ok(None)
        } else {
            Ok(Some(branch))
        }
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

fn parse_worktree_list(output: &str) -> Vec<GitWorkspaceInfo> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree if exists
            if let Some(path) = current_path.take() {
                worktrees.push(GitWorkspaceInfo {
                    path,
                    branch: current_branch.take(),
                    ..Default::default()
                });
            }
            // Start new worktree
            current_path = Some(PathBuf::from(line.trim_start_matches("worktree ")));
        } else if line.starts_with("branch ") {
            let branch_ref = line.trim_start_matches("branch ");
            current_branch = branch_ref
                .strip_prefix("refs/heads/")
                .map(|s| s.to_string());
        } else if line.is_empty() {
            // Empty line separates worktree entries
            if let Some(path) = current_path.take() {
                worktrees.push(GitWorkspaceInfo {
                    path,
                    branch: current_branch.take(),
                    ..Default::default()
                });
            }
        }
    }

    // Handle last worktree if no trailing empty line
    if let Some(path) = current_path {
        worktrees.push(GitWorkspaceInfo {
            path,
            branch: current_branch,
            ..Default::default()
        });
    }

    worktrees
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

    #[test]
    fn test_parse_worktree_list_multiple_worktrees() {
        let output = "\
worktree /path/to/main
HEAD abc123def456
branch refs/heads/main

worktree /path/to/feature
HEAD 789ghi012jkl
branch refs/heads/feature-branch

";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(worktrees[0].kind, WorkspaceKind::Worktree);
        assert_eq!(worktrees[1].path, PathBuf::from("/path/to/feature"));
        assert_eq!(worktrees[1].branch, Some("feature-branch".to_string()));
        assert_eq!(worktrees[1].kind, WorkspaceKind::Worktree);
    }

    #[test]
    fn test_parse_worktree_list_detached_head() {
        let output = "\
worktree /path/to/detached
HEAD abc123def456
detached

";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/detached"));
        assert_eq!(worktrees[0].branch, None);
    }

    #[test]
    fn test_parse_worktree_list_empty_output() {
        let output = "";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 0);
    }

    #[test]
    fn test_parse_worktree_list_no_trailing_newline() {
        let output = "worktree /path/to/main
HEAD abc123def456
branch refs/heads/main";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
    }
}
