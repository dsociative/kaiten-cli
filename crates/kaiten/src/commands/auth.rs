use crate::cli::AuthCmd;
use crate::error::CliError;

pub async fn run(_cmd: AuthCmd, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
