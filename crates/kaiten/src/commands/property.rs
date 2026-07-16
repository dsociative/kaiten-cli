use kaiten_client::KaitenClient;

use crate::cli::PropertyCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: PropertyCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        PropertyCmd::List => {
            let props = client.properties().list().await?;
            if json {
                return output::print_json(&props);
            }
            let mut table = output::table(&["ID", "NAME", "TYPE"]);
            for prop in &props {
                table.add_row(vec![
                    prop.id.to_string(),
                    prop.name.clone(),
                    prop.property_type.clone(),
                ]);
            }
            println!("{table}");
            Ok(())
        }
        PropertyCmd::Values { property_id } => {
            let values = client.properties().select_values(property_id).await?;
            if json {
                return output::print_json(&values);
            }
            let mut table = output::table(&["ID", "VALUE"]);
            for value in &values {
                table.add_row(vec![value.id.to_string(), value.value.clone()]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
