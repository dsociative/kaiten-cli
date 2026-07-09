use crate::cli::McpCmd;
use crate::error::CliError;

pub async fn run(cmd: McpCmd) -> Result<(), CliError> {
    match cmd {
        McpCmd::Serve => Err(CliError::InvalidArg("not implemented yet".into())),
    }
}
