use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};

#[derive(Parser, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: Shell,
}

pub fn run(args: CompletionsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = crate::Cli::command();
    generate(args.shell, &mut cmd, "silo", &mut std::io::stdout());
    Ok(())
}
