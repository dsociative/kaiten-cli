use kaiten_client::KaitenClient;

use crate::cli::CardCmd;
use crate::config::Defaults;
use crate::error::CliError;

pub async fn run(
    _cmd: CardCmd,
    _client: &KaitenClient,
    _defaults: &Defaults,
    _json: bool,
) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
