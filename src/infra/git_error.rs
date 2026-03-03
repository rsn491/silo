use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("not a git repository")]
    NotAGitRepo,
    #[error("git error: {0}")]
    CommandFailed(String),
    #[error("worktree creation failed: {0}")]
    WorktreeCreationFailed(String),
    #[error("worktree removal failed: {0}")]
    WorktreeRemovalFailed(String),
    #[error("clone failed: {0}")]
    CloneFailed(String),
}
