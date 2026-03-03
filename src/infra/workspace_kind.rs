use std::fmt;

#[derive(
    Debug, Clone, PartialEq, Default, clap::ValueEnum, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKind {
    #[default]
    Worktree,
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
