use crate::infra::git_error::GitError;
use std::fmt;

#[derive(Debug)]
pub enum LaunchError {
    AgentSpawnError(String),
    Git(GitError),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::AgentSpawnError(msg) => write!(f, "failed to spawn agent: {}", msg),
            LaunchError::Git(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LaunchError::Git(err) => Some(err),
            LaunchError::AgentSpawnError(_) => None,
        }
    }
}

impl From<GitError> for LaunchError {
    fn from(err: GitError) -> Self {
        LaunchError::Git(err)
    }
}

pub trait AgentLauncher {
    fn launch(&self) -> Result<(), LaunchError>;
}
