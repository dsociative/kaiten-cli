use clap::CommandFactory;

use crate::cli::{Cli, Shell};

/// Generating a shell completion script can't fail, so unlike the other
/// command `run` functions this one returns nothing rather than a `Result`.
pub fn run(shell: Shell) {
    let shell = match shell {
        Shell::Bash => clap_complete::Shell::Bash,
        Shell::Zsh => clap_complete::Shell::Zsh,
        Shell::Fish => clap_complete::Shell::Fish,
    };
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "kaiten", &mut std::io::stdout());
}
