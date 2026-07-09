use kaiten_client::KaitenClient;

use crate::cli::SpaceCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: SpaceCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        SpaceCmd::List => {
            let spaces = client.spaces().list().await?;
            if json {
                return output::print_json(&spaces);
            }
            let mut table = output::table(&["ID", "TITLE"]);
            for space in &spaces {
                table.add_row(vec![space.id.to_string(), space.title.clone()]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
