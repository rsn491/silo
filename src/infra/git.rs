//! Traits and implementations for Git operations.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::infra::git_error::GitError;
use crate::infra::workspace_kind::WorkspaceKind;

/// Information about a Git workspace (worktree or clone).
#[derive(Debug, Clone, Default)]
pub struct GitWorkspaceInfo {
    /// The absolute path to the workspace directory.
    pub path: PathBuf,
    /// The name of the current branch in this workspace, if any.
    pub branch: Option<String>,
    /// Whether the workspace is a worktree or a full checkout.
    pub kind: WorkspaceKind,
    /// True when the workspace has uncommitted changes.
    pub has_uncommitted_changes: bool,
    /// Count of uncommitted files reported by `git status`.
    pub uncommitted_file_count: usize,
    /// Commits the workspace is ahead of the base branch.
    pub commits_ahead: usize,
    /// Commits the workspace is behind the base branch.
    pub commits_behind: usize,
}

/// Trait defining Git operations required by the service layer.
#[cfg_attr(test, mockall::automock)]
pub trait GitOperations {
    /// Returns the absolute path to the root of the current Git repository.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::NotAGitRepo`] if the current directory is not in a Git repository.
    fn get_repo_root(&self) -> Result<PathBuf, GitError>;

    /// Determines the project name, usually from the origin URL or directory name.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if the project name cannot be determined.
    fn get_project_name(&self) -> Result<String, GitError>;

    /// Creates a new Git worktree at the specified path for the given branch.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::WorktreeCreationFailed`] if the `git worktree add` command fails.
    fn create_worktree(&self, path: &Path, branch: &str) -> Result<(), GitError>;

    /// Lists all worktrees associated with the current Git repository.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if the `git worktree list` command fails.
    fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError>;

    /// Forcefully removes a Git worktree at the specified path.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::WorktreeRemovalFailed`] if the `git worktree remove` command fails.
    fn remove_worktree(&self, path: &Path) -> Result<(), GitError>;

    /// Returns the default remote branch name (e.g., "origin/main").
    ///
    /// # Errors
    ///
    /// Returns a default value if the command fails, or [`GitError::CommandFailed`] on critical errors.
    fn get_default_remote_branch(&self) -> Result<String, GitError>;

    /// Returns the output of `git status --porcelain` for the specified worktree.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if the command fails.
    fn get_status_porcelain(&self, worktree_path: &Path) -> Result<String, GitError>;

    /// Counts how many commits the current worktree is ahead of the base branch.
    ///
    /// # Errors
    ///
    /// Returns 0 if the command fails or if there's no remote.
    fn count_commits_ahead(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError>;

    /// Counts how many commits the current worktree is behind the base branch.
    ///
    /// # Errors
    ///
    /// Returns 0 if the command fails or if there's no remote.
    fn count_commits_behind(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError>;

    /// Performs a local clone of the source repository to the destination path.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CloneFailed`] if the `git clone --local` command fails.
    fn clone_local(&self, source: &Path, dest: &Path) -> Result<(), GitError>;

    /// Checks out a new branch in the specified repository path.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if the `git checkout -b` command fails.
    fn checkout_new_branch(&self, path: &Path, branch: &str) -> Result<(), GitError>;

    /// Returns the name of the current branch at the specified path.
    ///
    /// # Errors
    ///
    /// Returns [`Ok(None)`] if in detached HEAD state, or [`GitError::CommandFailed`] on error.
    fn get_current_branch(&self, path: &Path) -> Result<Option<String>, GitError>;

    /// Stages all changes in the working tree (`git add -A`).
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommitFailed`] if staging fails.
    fn stage_all(&self, path: &Path) -> Result<(), GitError>;

    /// Stages all changes and creates a commit with the given message.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommitFailed`] if staging or committing fails.
    fn commit_all(&self, path: &Path, message: &str) -> Result<(), GitError>;

    /// Pushes the current branch to its remote.
    /// If no upstream is set, attempts `git push --set-upstream origin <branch>`.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::PushFailed`] if the push fails.
    fn push(&self, path: &Path) -> Result<(), GitError>;

    /// Returns a human-readable summary of the current git changes in the workspace.
    ///
    /// Includes uncommitted file list, diff stat against HEAD, and recent commits. Used to
    /// provide context when generating a descriptive branch name.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if a git command fails.
    fn get_changes_summary(&self, path: &Path) -> Result<String, GitError>;

    /// Renames the current branch at the specified path.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if `git branch -m` fails.
    fn rename_branch(&self, path: &Path, new_name: &str) -> Result<(), GitError>;
}

/// A concrete implementation of [`GitOperations`] using the `git` command-line tool.
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
            // Fallback to origin/main if command fails.
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

    fn stage_all(&self, path: &Path) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["add", "-A"])
            .output()
            .map_err(|e| GitError::CommitFailed(e.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommitFailed(stderr.to_string()));
        }
        Ok(())
    }

    fn commit_all(&self, path: &Path, message: &str) -> Result<(), GitError> {
        self.stage_all(path)?;

        let commit_output = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["commit", "-m", message])
            .output()
            .map_err(|e| GitError::CommitFailed(e.to_string()))?;
        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            return Err(GitError::CommitFailed(stderr.to_string()));
        }
        Ok(())
    }

    fn push(&self, path: &Path) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["push"])
            .output()
            .map_err(|e| GitError::PushFailed(e.to_string()))?;

        if output.status.success() {
            return Ok(());
        }

        // Fallback: if no upstream is set, push with --set-upstream.
        let branch = self
            .get_current_branch(path)?
            .ok_or_else(|| GitError::PushFailed("cannot push from detached HEAD".to_string()))?;
        let output2 = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["push", "--set-upstream", "origin", &branch])
            .output()
            .map_err(|e| GitError::PushFailed(e.to_string()))?;
        if !output2.status.success() {
            let stderr = String::from_utf8_lossy(&output2.stderr);
            return Err(GitError::PushFailed(stderr.to_string()));
        }
        Ok(())
    }

    fn get_changes_summary(&self, path: &Path) -> Result<String, GitError> {
        let mut parts: Vec<String> = Vec::new();

        // Uncommitted file list.
        let status = self.get_status_porcelain(path)?;
        if !status.trim().is_empty() {
            parts.push(format!("Uncommitted changes:\n{}", status.trim()));
        }

        // Diff stat against HEAD (covers both staged and unstaged changes).
        let diff_stat = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["diff", "--stat", "HEAD"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;
        if diff_stat.status.success() {
            let stat = String::from_utf8_lossy(&diff_stat.stdout)
                .trim()
                .to_string();
            if !stat.is_empty() {
                parts.push(format!("Diff stat:\n{}", stat));
            }
        }

        // Recent commits for additional context.
        let log = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["log", "--oneline", "-5"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;
        if log.status.success() {
            let log_text = String::from_utf8_lossy(&log.stdout).trim().to_string();
            if !log_text.is_empty() {
                parts.push(format!("Recent commits:\n{}", log_text));
            }
        }

        Ok(parts.join("\n\n"))
    }

    fn rename_branch(&self, path: &Path, new_name: &str) -> Result<(), GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["branch", "-m", new_name])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        Ok(())
    }
}

/// Extracts the project name from a Git repository URL.
fn extract_project_name_from_url(url: &str) -> Option<String> {
    // Handle SSH format: git@github.com:user/repo.git.
    // Handle HTTPS format: https://github.com/user/repo.git.
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

/// Parses the output of `git worktree list --porcelain`.
fn parse_worktree_list(output: &str) -> Vec<GitWorkspaceInfo> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree if exists.
            if let Some(path) = current_path.take() {
                worktrees.push(GitWorkspaceInfo {
                    path,
                    branch: current_branch.take(),
                    ..Default::default()
                });
            }
            // Start new worktree.
            current_path = Some(PathBuf::from(line.trim_start_matches("worktree ")));
        } else if line.starts_with("branch ") {
            let branch_ref = line.trim_start_matches("branch ");
            current_branch = branch_ref
                .strip_prefix("refs/heads/")
                .map(|s| s.to_string());
        } else if line.is_empty() {
            // Empty line separates worktree entries.
            if let Some(path) = current_path.take() {
                worktrees.push(GitWorkspaceInfo {
                    path,
                    branch: current_branch.take(),
                    ..Default::default()
                });
            }
        }
    }

    // Handle last worktree if no trailing empty line.
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
