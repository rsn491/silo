//! Agent operation modes.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

/// Represents the mode in which an AI agent operates.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    EnumString,
    Display,
    EnumIter,
    IntoStaticStr,
    Serialize,
    Deserialize,
    Default,
)]
#[strum(serialize_all = "lowercase")]
pub enum AgentMode {
    /// The agent focuses on planning.
    #[default]
    Plan,
    /// The agent focuses on writing code.
    Code,
}
