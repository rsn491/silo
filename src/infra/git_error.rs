use std::fmt;

#[derive(Debug)]
pub enum GitError {
    NotAGitRepo,
    GitError(String),
    WorktreeCreationFailed(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::NotAGitRepo => write!(f, "not a git repository"),
            GitError::GitError(msg) => write!(f, "git error: {}", msg),
            GitError::WorktreeCreationFailed(msg) => write!(f, "worktree creation failed: {}", msg),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
