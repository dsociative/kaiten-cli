use crate::cli::Shell;
use crate::error::CliError;

pub fn run(_shell: Shell) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
