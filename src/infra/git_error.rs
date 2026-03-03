//! Error types for Git operations.

use thiserror::Error;

/// Represents errors that can occur during Git operations.
#[derive(Debug, Error)]
pub enum GitError {
    /// The current directory is not part of a Git repository.
    #[error("not a git repository")]
    NotAGitRepo,
    /// A Git command failed with the specified error message.
    #[error("git error: {0}")]
    CommandFailed(String),
    /// Failed to create a new Git worktree.
    #[error("worktree creation failed: {0}")]
    WorktreeCreationFailed(String),
    /// Failed to remove a Git worktree.
    #[error("worktree removal failed: {0}")]
    WorktreeRemovalFailed(String),
    /// Failed to clone a repository.
    #[error("clone failed: {0}")]
    CloneFailed(String),
}
