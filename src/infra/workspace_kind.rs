use strum::Display;

#[derive(
    Debug, Clone, PartialEq, Default, Display, clap::ValueEnum, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
/// Identifies the type of workspace.
pub enum WorkspaceKind {
    #[default]
    /// A Git worktree attached to the main repository.
    #[strum(to_string = "worktree")]
    Worktree,
    /// A full checkout created via local clone.
    #[strum(to_string = "checkout")]
    Checkout,
}
