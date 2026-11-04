//! OpenCode agent implementation.

use super::{AgentCommand, PromptError, run_headless};

/// Concrete implementation of [`AgentCommand`] for OpenCode.
pub(super) struct OpenCodeAgent;

impl AgentCommand for OpenCodeAgent {
    fn command_name(&self) -> &'static str {
        "opencode"
    }

    fn prompt(&self, message: &str) -> Result<String, PromptError> {
        run_headless("opencode", &["-p", message])
    }
}
