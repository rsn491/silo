//! Traits and implementations for Git operations.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::infra::git_error::GitError;

/// Information about a Git workspace (worktree or clone).
#[derive(Debug, Clone, Default)]
pub struct GitWorkspaceInfo {
    /// The absolute path to the workspace directory.
    pub path: PathBuf,
    /// The name of the current branch in this workspace, if any.
    pub branch: Option<String>,
    /// True when the workspace has uncommitted changes.
    pub has_uncommitted_changes: bool,
    /// Commits the workspace is ahead of the base branch.
    pub commits_ahead: usize,
    /// The latest commit summary (short hash + message), if available.
    pub latest_commit: Option<String>,
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

    /// Returns the local filesystem path configured as `origin` for `repository_path`.
    /// Non-local remotes and missing origins return `Ok(None)`.
    fn get_local_origin(&self, repository_path: &Path) -> Result<Option<PathBuf>, GitError>;

    /// Returns the primary worktree for the repository containing `repository_path`.
    fn get_primary_worktree(&self, repository_path: &Path) -> Result<PathBuf, GitError>;

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

    /// Checks out a new branch starting from `start_point` in the specified path.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if the `git checkout -b` command fails.
    fn checkout_new_branch_from(
        &self,
        path: &Path,
        branch: &str,
        start_point: &str,
    ) -> Result<(), GitError>;

    /// Fetches from origin in the specified repository path.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if the `git fetch` command fails.
    fn fetch_remote(&self, path: &Path) -> Result<(), GitError>;

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

    /// Returns a one-line summary of the latest commit at the specified path.
    ///
    /// Returns `None` if there are no commits or the command fails.
    fn get_latest_commit(&self, path: &Path) -> Result<Option<String>, GitError>;

    /// Returns `true` when the current branch at `path` has an upstream tracking
    /// branch and all local commits have been pushed (i.e. `@{u}..HEAD` is empty).
    ///
    /// Returns `false` if there is no upstream, or the command fails.
    fn is_fully_pushed(&self, path: &Path) -> bool;
}

/// Default fallback remote branch used when the remote HEAD cannot be determined.
const DEFAULT_REMOTE_BRANCH: &str = "origin/main";

/// A concrete implementation of [`GitOperations`] using the `git` command-line tool.
#[derive(Default, Clone)]
pub struct Git {
    /// Directory used for commands that operate on "the current repository".
    ///
    /// `None` means the process working directory.
    work_dir: Option<PathBuf>,
}

impl Git {
    /// Creates a `Git` that operates on the repository of the process working directory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `Git` that operates on the repository containing `dir`.
    #[cfg(test)]
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            work_dir: Some(dir.into()),
        }
    }

    /// Returns a `git` command rooted at the configured working directory.
    fn command(&self) -> Command {
        let mut command = Command::new("git");
        if let Some(dir) = &self.work_dir {
            command.current_dir(dir);
        }
        command
    }
}

impl GitOperations for Git {
    fn get_repo_root(&self) -> Result<PathBuf, GitError> {
        let output = self
            .command()
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
        let output = self
            .command()
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
        let output = self
            .command()
            .args(["worktree", "add", "-b", branch])
            .arg(path)
            .output()
            .map_err(|e| GitError::WorktreeCreationFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Branch already exists but has no worktree — check it out directly.
            if stderr.contains("already exists") {
                let retry = self
                    .command()
                    .args(["worktree", "add"])
                    .arg(path)
                    .arg(branch)
                    .output()
                    .map_err(|e| GitError::WorktreeCreationFailed(e.to_string()))?;

                if !retry.status.success() {
                    let retry_stderr = String::from_utf8_lossy(&retry.stderr);
                    return Err(GitError::WorktreeCreationFailed(retry_stderr.to_string()));
                }
                return Ok(());
            }
            return Err(GitError::WorktreeCreationFailed(stderr.to_string()));
        }

        Ok(())
    }

    fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
        let output = self
            .command()
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

    fn get_local_origin(&self, repository_path: &Path) -> Result<Option<PathBuf>, GitError> {
        let output = self
            .command()
            .args(["-C"])
            .arg(repository_path)
            .args(["remote", "get-url", "origin"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let path = if let Some(path) = value.strip_prefix("file://") {
            PathBuf::from(path)
        } else {
            let candidate = PathBuf::from(&value);
            if !candidate.is_absolute() && (value.contains("://") || value.contains(':')) {
                return Ok(None);
            }
            if candidate.is_absolute() {
                candidate
            } else {
                repository_path.join(candidate)
            }
        };

        if !path.exists() {
            return Ok(None);
        }

        Ok(Some(path.canonicalize().unwrap_or(path)))
    }

    fn get_primary_worktree(&self, repository_path: &Path) -> Result<PathBuf, GitError> {
        let output = self
            .command()
            .args(["-C"])
            .arg(repository_path)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.trim().to_string()));
        }

        parse_worktree_list(&String::from_utf8_lossy(&output.stdout))
            .into_iter()
            .next()
            .map(|workspace| workspace.path)
            .ok_or_else(|| {
                GitError::CommandFailed("repository has no primary worktree".to_string())
            })
    }

    fn remove_worktree(&self, path: &Path) -> Result<(), GitError> {
        let output = self
            .command()
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
        let output = self
            .command()
            .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout)
                .trim()
                .strip_prefix("refs/remotes/")
                .unwrap_or(DEFAULT_REMOTE_BRANCH)
                .to_string();
            Ok(branch)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!(
                "could not determine default remote branch ({}); falling back to {}",
                stderr.trim(),
                DEFAULT_REMOTE_BRANCH
            );
            Ok(DEFAULT_REMOTE_BRANCH.to_string())
        }
    }

    fn get_status_porcelain(&self, worktree_path: &Path) -> Result<String, GitError> {
        let output = self
            .command()
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
        let output = self
            .command()
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(GitError::CommandFailed(stderr.trim().to_string()))
        }
    }

    fn count_commits_behind(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<usize, GitError> {
        let output = self
            .command()
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(GitError::CommandFailed(stderr.trim().to_string()))
        }
    }

    fn clone_local(&self, source: &Path, dest: &Path) -> Result<(), GitError> {
        let output = self
            .command()
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
        let output = self
            .command()
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

    fn checkout_new_branch_from(
        &self,
        path: &Path,
        branch: &str,
        start_point: &str,
    ) -> Result<(), GitError> {
        let output = self
            .command()
            .args(["-C"])
            .arg(path)
            .args(["checkout", "-b", branch, start_point])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        Ok(())
    }

    fn fetch_remote(&self, path: &Path) -> Result<(), GitError> {
        let output = self
            .command()
            .args(["-C"])
            .arg(path)
            .args(["fetch", "origin"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(stderr.to_string()));
        }

        Ok(())
    }

    fn get_current_branch(&self, path: &Path) -> Result<Option<String>, GitError> {
        let output = self
            .command()
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
        let output = self
            .command()
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

        let commit_output = self
            .command()
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
        let output = self
            .command()
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
        let output2 = self
            .command()
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
        let diff_stat = self
            .command()
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
        let log = self
            .command()
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
        let output = self
            .command()
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

    fn get_latest_commit(&self, path: &Path) -> Result<Option<String>, GitError> {
        let output = self
            .command()
            .args(["-C"])
            .arg(path)
            .args(["log", "--oneline", "-1"])
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if commit.is_empty() {
            Ok(None)
        } else {
            Ok(Some(commit))
        }
    }

    fn is_fully_pushed(&self, path: &Path) -> bool {
        let output = self
            .command()
            .args(["-C"])
            .arg(path)
            .args(["rev-list", "--count", "@{u}..HEAD"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let count = String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse::<usize>()
                    .unwrap_or(1);
                count == 0
            }
            _ => false,
        }
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
        // Arrange
        let output = "\
worktree /path/to/main
HEAD abc123def456
branch refs/heads/main

worktree /path/to/feature
HEAD 789ghi012jkl
branch refs/heads/feature-branch

";

        // Act
        let worktrees = parse_worktree_list(output);

        // Assert
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(worktrees[1].path, PathBuf::from("/path/to/feature"));
        assert_eq!(worktrees[1].branch, Some("feature-branch".to_string()));
    }

    #[test]
    fn test_parse_worktree_list_detached_head() {
        // Arrange
        let output = "\
worktree /path/to/detached
HEAD abc123def456
detached

";

        // Act
        let worktrees = parse_worktree_list(output);

        // Assert
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
        // Arrange
        let output = "worktree /path/to/main
HEAD abc123def456
branch refs/heads/main";

        // Act
        let worktrees = parse_worktree_list(output);

        // Assert
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
    }
}

/// Validates a branch name against a safe allowlist: `[a-zA-Z0-9._/\-]+`.
///
/// Rejects any name containing characters outside this set to prevent shell/AppleScript
/// injection when branch names are embedded in commands passed to iTerm2 or other terminals.
///
/// # Errors
///
/// Returns a `String` describing the first invalid character, or that the name is empty.
pub fn validate_branch_name(branch: &str) -> Result<(), String> {
    if branch.is_empty() {
        return Err("branch name must not be empty".to_string());
    }
    let invalid_char = branch
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '/' | '-'));
    if let Some(c) = invalid_char {
        return Err(format!(
            "branch name contains invalid character {:?}; only a-z, A-Z, 0-9, '.', '_', '/', '-' are allowed",
            c
        ));
    }
    Ok(())
}

#[cfg(test)]
mod branch_name_tests {
    use super::*;

    #[test]
    fn test_validate_branch_name_accepts_valid_names() {
        let valid = [
            "main",
            "feature/my-branch",
            "fix-123",
            "release/1.0.0",
            "user_feature",
            "UPPER/lower_123",
            "a",
        ];
        for name in valid {
            assert!(
                validate_branch_name(name).is_ok(),
                "should accept {:?}",
                name
            );
        }
    }

    #[test]
    fn test_validate_branch_name_rejects_injection_attempts() {
        let invalid = [
            "feature'; do shell script \"rm -rf ~\"",
            "branch$(whoami)",
            "branch`id`",
            "branch name",
            "branch\nnewline",
            "branch!bang",
            "",
        ];
        for name in invalid {
            assert!(
                validate_branch_name(name).is_err(),
                "should reject {:?}",
                name
            );
        }
    }
}

#[cfg(test)]
pub mod test_support {
    //! Throwaway Git repositories for tests.

    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tempfile::TempDir;

    use super::Git;

    /// Runs `git` in `dir`, panicking when the command fails.
    pub fn run_git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Configures a commit identity and disables signing for a test repository.
    pub fn configure_identity(dir: &Path) {
        run_git(dir, &["config", "user.name", "Silo Test"]);
        run_git(dir, &["config", "user.email", "silo@example.com"]);
        run_git(dir, &["config", "commit.gpgsign", "false"]);
    }

    /// Writes `contents` to `name` in `dir` and commits it.
    pub fn commit_file(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).unwrap();
        run_git(dir, &["add", "-A"]);
        run_git(dir, &["commit", "-m", &format!("add {}", name)]);
    }

    /// A bare `origin` repository plus a clone of it with one pushed commit on `main`.
    pub struct TestRepo {
        /// Owns every path below.
        pub tmp: TempDir,
        /// Project name derived from the origin URL (unique per repository).
        pub project: String,
        /// Path to the bare origin repository.
        pub origin: PathBuf,
        /// Path to the working clone.
        pub work: PathBuf,
    }

    impl TestRepo {
        /// Creates the origin and a clone with one pushed commit.
        pub fn new() -> Self {
            let tmp = TempDir::new().unwrap();
            let base = tmp.path().canonicalize().unwrap();
            let project = format!(
                "silo-test-{}",
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            );
            let origin_name = format!("{}.git", project);
            let origin = base.join(&origin_name);
            let work = base.join("work");
            run_git(&base, &["init", "--bare", "-b", "main", &origin_name]);
            run_git(&base, &["clone", &origin_name, "work"]);
            configure_identity(&work);
            run_git(&work, &["checkout", "-B", "main"]);
            commit_file(&work, "README.md", "hello\n");
            run_git(&work, &["push", "-u", "origin", "main"]);
            run_git(&work, &["remote", "set-head", "origin", "main"]);
            Self {
                tmp,
                project,
                origin,
                work,
            }
        }

        /// Path to a not-yet-existing directory next to the repositories.
        pub fn sibling(&self, name: &str) -> PathBuf {
            self.tmp.path().canonicalize().unwrap().join(name)
        }

        /// A [`Git`] rooted at the working clone.
        pub fn git(&self) -> Git {
            Git::in_dir(&self.work)
        }
    }

    /// Creates a standalone repository with one commit and no remotes.
    pub fn standalone_repo() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().canonicalize().unwrap();
        run_git(&path, &["init", "-b", "main"]);
        configure_identity(&path);
        commit_file(&path, "file.txt", "content\n");
        (tmp, path)
    }

    /// A directory that is guaranteed not to be inside a Git repository.
    pub fn not_a_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(tmp.path())
                .args(["rev-parse", "--git-dir"])
                .output()
                .unwrap()
                .status
                .success(),
            "temp dir unexpectedly inside a git repository"
        );
        tmp
    }
}

#[cfg(test)]
mod git_cli_tests {
    use super::test_support::*;
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn get_repo_root_returns_top_level_from_subdirectory() {
        let repo = TestRepo::new();
        let sub = repo.work.join("nested");
        fs::create_dir_all(&sub).unwrap();

        let root = Git::in_dir(&sub).get_repo_root().unwrap();

        assert_eq!(root, repo.work);
    }

    #[test]
    fn get_repo_root_fails_outside_repository() {
        let dir = not_a_repo();

        let result = Git::in_dir(dir.path()).get_repo_root();

        assert!(matches!(result, Err(GitError::NotAGitRepo)));
    }

    #[test]
    fn get_project_name_uses_origin_url() {
        let repo = TestRepo::new();

        assert_eq!(repo.git().get_project_name().unwrap(), repo.project);
    }

    #[test]
    fn get_project_name_falls_back_to_directory_name() {
        let (_tmp, path) = standalone_repo();

        let name = Git::in_dir(&path).get_project_name().unwrap();

        assert_eq!(name, path.file_name().unwrap().to_str().unwrap());
    }

    #[test]
    fn get_project_name_fails_outside_repository() {
        let dir = not_a_repo();

        assert!(Git::in_dir(dir.path()).get_project_name().is_err());
    }

    #[test]
    fn extract_project_name_rejects_empty_url() {
        assert_eq!(extract_project_name_from_url(""), None);
    }

    #[test]
    fn create_list_and_remove_worktree() {
        let repo = TestRepo::new();
        let git = repo.git();
        let wt_path = repo.sibling("wt-feature");

        git.create_worktree(&wt_path, "feature").unwrap();
        let worktrees = git.list_worktrees().unwrap();

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, repo.work);
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature"));
        assert_eq!(git.get_primary_worktree(&wt_path).unwrap(), repo.work);

        git.remove_worktree(&wt_path).unwrap();
        assert_eq!(git.list_worktrees().unwrap().len(), 1);
        assert!(!wt_path.exists());
    }

    #[test]
    fn create_worktree_checks_out_existing_branch() {
        let repo = TestRepo::new();
        run_git(&repo.work, &["branch", "existing"]);
        let wt_path = repo.sibling("wt-existing");

        repo.git().create_worktree(&wt_path, "existing").unwrap();

        assert_eq!(
            run_git(&wt_path, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "existing"
        );
    }

    #[test]
    fn create_worktree_fails_when_branch_checked_out_elsewhere() {
        let repo = TestRepo::new();

        let result = repo.git().create_worktree(&repo.sibling("wt-main"), "main");

        assert!(matches!(result, Err(GitError::WorktreeCreationFailed(_))));
    }

    #[test]
    fn create_worktree_fails_outside_repository() {
        let dir = not_a_repo();

        let result = Git::in_dir(dir.path()).create_worktree(&dir.path().join("wt"), "branch");

        assert!(matches!(result, Err(GitError::WorktreeCreationFailed(_))));
    }

    #[test]
    fn list_worktrees_fails_outside_repository() {
        let dir = not_a_repo();

        assert!(matches!(
            Git::in_dir(dir.path()).list_worktrees(),
            Err(GitError::CommandFailed(_))
        ));
    }

    #[test]
    fn get_primary_worktree_fails_outside_repository() {
        let dir = not_a_repo();

        assert!(Git::new().get_primary_worktree(dir.path()).is_err());
    }

    #[test]
    fn remove_worktree_fails_for_unknown_path() {
        let repo = TestRepo::new();

        let result = repo.git().remove_worktree(&repo.sibling("missing"));

        assert!(matches!(result, Err(GitError::WorktreeRemovalFailed(_))));
    }

    #[test]
    fn get_local_origin_resolves_absolute_path() {
        let repo = TestRepo::new();

        let origin = Git::new().get_local_origin(&repo.work).unwrap();

        assert_eq!(origin, Some(repo.origin.clone()));
    }

    #[test]
    fn get_local_origin_resolves_relative_and_file_urls() {
        let repo = TestRepo::new();
        let git = Git::new();

        run_git(
            &repo.work,
            &[
                "remote",
                "set-url",
                "origin",
                &format!("../{}.git", repo.project),
            ],
        );
        assert_eq!(
            git.get_local_origin(&repo.work).unwrap(),
            Some(repo.origin.clone())
        );

        let file_url = format!("file://{}", repo.origin.display());
        run_git(&repo.work, &["remote", "set-url", "origin", &file_url]);
        assert_eq!(
            git.get_local_origin(&repo.work).unwrap(),
            Some(repo.origin.clone())
        );
    }

    #[test]
    fn get_local_origin_ignores_remote_and_missing_origins() {
        let repo = TestRepo::new();
        let git = Git::new();

        run_git(
            &repo.work,
            &[
                "remote",
                "set-url",
                "origin",
                "git@github.com:user/repo.git",
            ],
        );
        assert_eq!(git.get_local_origin(&repo.work).unwrap(), None);

        let missing = repo.sibling("does-not-exist.git");
        run_git(
            &repo.work,
            &["remote", "set-url", "origin", missing.to_str().unwrap()],
        );
        assert_eq!(git.get_local_origin(&repo.work).unwrap(), None);

        let (_tmp, standalone) = standalone_repo();
        assert_eq!(git.get_local_origin(&standalone).unwrap(), None);
    }

    #[test]
    fn get_default_remote_branch_reads_origin_head() {
        let repo = TestRepo::new();

        assert_eq!(
            repo.git().get_default_remote_branch().unwrap(),
            "origin/main"
        );
    }

    #[test]
    fn get_default_remote_branch_falls_back_without_origin_head() {
        let (_tmp, path) = standalone_repo();

        assert_eq!(
            Git::in_dir(&path).get_default_remote_branch().unwrap(),
            DEFAULT_REMOTE_BRANCH
        );
    }

    #[test]
    fn status_and_commit_flow() {
        let repo = TestRepo::new();
        let git = Git::new();
        assert_eq!(git.get_status_porcelain(&repo.work).unwrap(), "");

        fs::write(repo.work.join("new.txt"), "new\n").unwrap();
        let status = git.get_status_porcelain(&repo.work).unwrap();
        assert!(status.contains("new.txt"));

        git.commit_all(&repo.work, "add new file").unwrap();
        assert_eq!(git.get_status_porcelain(&repo.work).unwrap(), "");
        assert!(
            git.get_latest_commit(&repo.work)
                .unwrap()
                .unwrap()
                .ends_with("add new file")
        );
    }

    #[test]
    fn status_fails_for_missing_directory() {
        let repo = TestRepo::new();

        let result = Git::new().get_status_porcelain(&repo.sibling("missing"));

        assert!(matches!(result, Err(GitError::CommandFailed(_))));
    }

    #[test]
    fn commit_all_fails_when_nothing_to_commit() {
        let repo = TestRepo::new();

        let result = Git::new().commit_all(&repo.work, "empty");

        assert!(matches!(result, Err(GitError::CommitFailed(_))));
    }

    #[test]
    fn stage_all_fails_for_missing_directory() {
        let repo = TestRepo::new();

        let result = Git::new().stage_all(&repo.sibling("missing"));

        assert!(matches!(result, Err(GitError::CommitFailed(_))));
    }

    #[test]
    fn count_commits_ahead_and_behind() {
        let repo = TestRepo::new();
        let git = Git::new();
        commit_file(&repo.work, "a.txt", "a\n");
        commit_file(&repo.work, "b.txt", "b\n");

        assert_eq!(
            git.count_commits_ahead(&repo.work, "origin/main").unwrap(),
            2
        );
        assert_eq!(
            git.count_commits_behind(&repo.work, "origin/main").unwrap(),
            0
        );

        run_git(&repo.work, &["checkout", "-q", "origin/main"]);
        assert_eq!(git.count_commits_ahead(&repo.work, "main").unwrap(), 0);
        assert_eq!(git.count_commits_behind(&repo.work, "main").unwrap(), 2);
    }

    #[test]
    fn count_commits_fails_for_unknown_base() {
        let repo = TestRepo::new();
        let git = Git::new();

        assert!(git.count_commits_ahead(&repo.work, "nope/nope").is_err());
        assert!(git.count_commits_behind(&repo.work, "nope/nope").is_err());
    }

    #[test]
    fn clone_local_and_fetch() {
        let repo = TestRepo::new();
        let git = Git::new();
        let dest = repo.sibling("clone");

        git.clone_local(&repo.origin, &dest).unwrap();
        git.fetch_remote(&dest).unwrap();

        assert!(dest.join("README.md").exists());
        assert!(matches!(
            git.clone_local(&repo.origin, &dest),
            Err(GitError::CloneFailed(_))
        ));
    }

    #[test]
    fn fetch_remote_fails_without_origin() {
        let (_tmp, path) = standalone_repo();

        assert!(Git::new().fetch_remote(&path).is_err());
    }

    #[test]
    fn checkout_new_branch_variants() {
        let repo = TestRepo::new();
        let git = Git::new();

        git.checkout_new_branch(&repo.work, "first").unwrap();
        assert_eq!(
            git.get_current_branch(&repo.work).unwrap().as_deref(),
            Some("first")
        );
        assert!(git.checkout_new_branch(&repo.work, "first").is_err());

        git.checkout_new_branch_from(&repo.work, "second", "origin/main")
            .unwrap();
        assert_eq!(
            git.get_current_branch(&repo.work).unwrap().as_deref(),
            Some("second")
        );
        assert!(
            git.checkout_new_branch_from(&repo.work, "third", "missing/ref")
                .is_err()
        );
    }

    #[test]
    fn get_current_branch_handles_detached_and_missing_repos() {
        let repo = TestRepo::new();
        let git = Git::new();
        run_git(&repo.work, &["checkout", "-q", "--detach"]);

        assert_eq!(git.get_current_branch(&repo.work).unwrap(), None);
        let dir = not_a_repo();
        assert_eq!(git.get_current_branch(dir.path()).unwrap(), None);
    }

    #[test]
    fn push_with_and_without_upstream() {
        let repo = TestRepo::new();
        let git = Git::new();

        commit_file(&repo.work, "pushed.txt", "x\n");
        assert!(!git.is_fully_pushed(&repo.work));
        git.push(&repo.work).unwrap();
        assert!(git.is_fully_pushed(&repo.work));

        git.checkout_new_branch(&repo.work, "topic").unwrap();
        commit_file(&repo.work, "topic.txt", "y\n");
        assert!(!git.is_fully_pushed(&repo.work), "no upstream yet");
        git.push(&repo.work).unwrap();
        assert!(git.is_fully_pushed(&repo.work));
        assert_eq!(
            run_git(&repo.origin, &["rev-parse", "--verify", "topic"]),
            run_git(&repo.work, &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn push_fails_from_detached_head() {
        let repo = TestRepo::new();
        run_git(&repo.work, &["checkout", "-q", "--detach"]);

        let result = Git::new().push(&repo.work);

        assert!(matches!(result, Err(GitError::PushFailed(_))));
    }

    #[test]
    fn push_fails_when_remote_is_unreachable() {
        let repo = TestRepo::new();
        let git = Git::new();
        git.checkout_new_branch(&repo.work, "topic").unwrap();
        let missing = repo.sibling("gone.git");
        run_git(
            &repo.work,
            &["remote", "set-url", "origin", missing.to_str().unwrap()],
        );

        assert!(matches!(git.push(&repo.work), Err(GitError::PushFailed(_))));
    }

    #[test]
    fn get_changes_summary_includes_all_sections() {
        let repo = TestRepo::new();
        fs::write(repo.work.join("README.md"), "changed\n").unwrap();

        let summary = Git::new().get_changes_summary(&repo.work).unwrap();

        assert!(summary.contains("Uncommitted changes:"));
        assert!(summary.contains("Diff stat:"));
        assert!(summary.contains("Recent commits:"));
    }

    #[test]
    fn get_changes_summary_propagates_status_error() {
        let repo = TestRepo::new();

        assert!(
            Git::new()
                .get_changes_summary(&repo.sibling("missing"))
                .is_err()
        );
    }

    #[test]
    fn rename_branch_success_and_failure() {
        let repo = TestRepo::new();
        let git = Git::new();
        run_git(&repo.work, &["branch", "taken"]);

        git.rename_branch(&repo.work, "renamed").unwrap();
        assert_eq!(
            git.get_current_branch(&repo.work).unwrap().as_deref(),
            Some("renamed")
        );
        assert!(git.rename_branch(&repo.work, "taken").is_err());
    }

    #[test]
    fn get_latest_commit_is_none_without_commits() {
        let tmp = TempDir::new().unwrap();
        run_git(tmp.path(), &["init", "-b", "main"]);

        assert_eq!(Git::new().get_latest_commit(tmp.path()).unwrap(), None);
    }
}
