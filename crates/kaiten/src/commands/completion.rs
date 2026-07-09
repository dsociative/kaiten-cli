use clap::CommandFactory;

use crate::cli::{Cli, Shell};
use crate::error::CliError;

pub fn run(shell: Shell) -> Result<(), CliError> {
    let shell = match shell {
        Shell::Bash => clap_complete::Shell::Bash,
        Shell::Zsh => clap_complete::Shell::Zsh,
        Shell::Fish => clap_complete::Shell::Fish,
    };
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "kaiten", &mut std::io::stdout());
    Ok(())
}
