use kaiten_client::KaitenClient;

use crate::error::CliError;

pub async fn run(
    _client: &KaitenClient,
    _method: &str,
    _path: &str,
    _data: Option<String>,
) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
