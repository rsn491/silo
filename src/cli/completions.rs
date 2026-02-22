use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};

#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: Shell,
}

pub struct CompletionsCommand;

impl CompletionsCommand {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, args: CompletionsArgs) -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = crate::Cli::command();
        generate(args.shell, &mut cmd, "silo", &mut std::io::stdout());
        Ok(())
    }
}
