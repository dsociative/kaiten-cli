use kaiten_client::KaitenClient;

use crate::cli::TagCmd;
use crate::error::CliError;

pub async fn run(_cmd: TagCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
