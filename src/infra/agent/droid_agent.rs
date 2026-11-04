//! Droid agent implementation.

use super::{AgentCommand, PromptError, run_headless};

/// Concrete implementation of [`AgentCommand`] for Factory.ai's Droid.
pub(super) struct DroidAgent;

impl AgentCommand for DroidAgent {
    fn command_name(&self) -> &'static str {
        "droid"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        run_headless("droid", &["exec", message])
    }
}
