use kaiten_client::KaitenClient;

use crate::cli::CardTypeCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: CardTypeCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        CardTypeCmd::List => {
            let types = client.tags().card_types().await?;
            if json {
                return output::print_json(&types);
            }
            let mut table = output::table(&["ID", "NAME", "LETTER"]);
            for t in &types {
                table.add_row(vec![
                    t.id.to_string(),
                    t.name.clone(),
                    t.letter.clone().unwrap_or_else(|| "-".to_string()),
                ]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
