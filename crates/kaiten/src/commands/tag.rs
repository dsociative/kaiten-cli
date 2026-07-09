use kaiten_client::KaitenClient;

use crate::cli::TagCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: TagCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        TagCmd::List => {
            let tags = client.tags().list().await?;
            if json {
                return output::print_json(&tags);
            }
            let mut table = output::table(&["ID", "NAME"]);
            for tag in &tags {
                table.add_row(vec![tag.id.to_string(), tag.name.clone()]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
