use kaiten_client::KaitenClient;

use crate::cli::BoardCmd;
use crate::config::Defaults;
use crate::error::CliError;
use crate::output;

pub async fn run(
    cmd: BoardCmd,
    client: &KaitenClient,
    defaults: &Defaults,
    json: bool,
) -> Result<(), CliError> {
    match cmd {
        BoardCmd::List { space } => {
            let space_id = space.or(defaults.space).ok_or_else(|| {
                CliError::InvalidArg("specify --space or set defaults.space in config".into())
            })?;
            let boards = client.boards().list(space_id).await?;
            if json {
                return output::print_json(&boards);
            }
            let mut table = output::table(&["ID", "TITLE"]);
            for board in &boards {
                table.add_row(vec![board.id.to_string(), board.title.clone()]);
            }
            println!("{table}");
            Ok(())
        }
        BoardCmd::View { board_id } => {
            let board = client.boards().get(board_id).await?;
            if json {
                return output::print_json(&board);
            }
            println!("Board {}: {}", board.id, board.title);
            println!();
            println!("Columns:");
            let mut columns = output::table(&["ID", "TITLE", "TYPE"]);
            for column in &board.columns {
                let type_label = match column.column_type {
                    Some(1) => "queued",
                    Some(2) => "in progress",
                    Some(3) => "done",
                    _ => "-",
                };
                columns.add_row(vec![
                    column.id.to_string(),
                    column.title.clone(),
                    type_label.to_string(),
                ]);
            }
            println!("{columns}");
            println!();
            println!("Lanes:");
            let mut lanes = output::table(&["ID", "TITLE"]);
            for lane in &board.lanes {
                lanes.add_row(vec![lane.id.to_string(), lane.title.clone()]);
            }
            println!("{lanes}");
            Ok(())
        }
    }
}
