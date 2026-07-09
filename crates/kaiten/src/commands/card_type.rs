use kaiten_client::KaitenClient;

use crate::cli::CardTypeCmd;
use crate::error::CliError;

pub async fn run(_cmd: CardTypeCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
