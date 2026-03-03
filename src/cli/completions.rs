//! Logic for the `completions` command.

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};

/// Arguments for the `completions` command.
#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: Shell,
}

/// Handler for the `completions` command.
pub struct CompletionsCommand;

impl CompletionsCommand {
    /// Creates a new `CompletionsCommand`.
    pub fn new() -> Self {
        Self
    }

    /// Executes the completions generation.
    ///
    /// # Errors
    ///
    /// Returns an error if the shell completion script cannot be generated.
    pub fn run(&self, args: CompletionsArgs) -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = crate::Cli::command();
        generate(args.shell, &mut cmd, "silo", &mut std::io::stdout());
        Ok(())
    }
}
