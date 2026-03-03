use std::fmt;

#[derive(
    Debug, Clone, PartialEq, Default, clap::ValueEnum, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
/// Identifies the type of workspace.
pub enum WorkspaceKind {
    #[default]
    /// A Git worktree attached to the main repository.
    Worktree,
    /// A full checkout created via local clone.
    Checkout,
}

impl fmt::Display for WorkspaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceKind::Worktree => write!(f, "worktree"),
            WorkspaceKind::Checkout => write!(f, "checkout"),
        }
    }
}
