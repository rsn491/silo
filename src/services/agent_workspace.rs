use std::path::PathBuf;

use crate::services::agent_launcher::LaunchError;

/// Trait for managing agent workspaces.
/// Implementers create isolated workspaces and return the path where an agent should be launched.
pub trait AgentWorkspace {
    /// Creates a new workspace and returns its path.
    fn create(&self) -> Result<PathBuf, LaunchError>;
}
