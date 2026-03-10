//! Infrastructure layer for Git operations and workspace management.

pub mod agent;
pub mod git;
pub mod git_error;
pub mod osascript;
pub mod system_process;
pub mod terminal;
/// Workspace classification types used across services.
pub mod workspace_kind;
