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
        self.write_to(args, &mut std::io::stdout());
        Ok(())
    }

    /// Writes the completion script for `args.shell` to `out`.
    fn write_to(&self, args: CompletionsArgs, out: &mut impl std::io::Write) {
        let mut cmd = crate::Cli::command();
        generate(args.shell, &mut cmd, "silo", out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_script_mentioning_subcommands() {
        // Given
        let shells = [Shell::Bash, Shell::Zsh, Shell::Fish];

        // When
        let scripts: Vec<String> = shells
            .iter()
            .map(|shell| {
                let mut out = Vec::new();
                CompletionsCommand::new().write_to(CompletionsArgs { shell: *shell }, &mut out);
                String::from_utf8(out).unwrap()
            })
            .collect();

        // Then
        for (shell, script) in shells.iter().zip(scripts) {
            assert!(script.contains("silo"), "{shell} script names the binary");
            assert!(
                script.contains("launch"),
                "{shell} script lists subcommands"
            );
        }
    }

    #[test]
    fn cli_definition_is_valid() {
        // Given
        let mut command = crate::Cli::command();

        // When
        command.build();

        // Then — clap panics on invalid definitions; the assertion makes that explicit.
        command.debug_assert();
    }
}
