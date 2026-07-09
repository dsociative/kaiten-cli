use kaiten_client::{CardFilter, KaitenClient};

use crate::cli::CardCmd;
use crate::config::Defaults;
use crate::error::CliError;
use crate::output;

pub async fn run(
    cmd: CardCmd,
    client: &KaitenClient,
    defaults: &Defaults,
    json: bool,
) -> Result<(), CliError> {
    match cmd {
        CardCmd::List {
            space,
            board,
            column,
            mine,
            member,
            query,
            tag,
            type_id,
            archived,
            limit,
        } => {
            let mut filter = CardFilter {
                limit: Some(limit),
                ..Default::default()
            };
            if board.is_none() && space.is_none() {
                if let Some(b) = defaults.board {
                    filter.board_id = Some(b);
                } else if let Some(s) = defaults.space {
                    filter.space_id = Some(s);
                } else {
                    return Err(CliError::InvalidArg(
                        "specify --board/--space or set defaults in config".into(),
                    ));
                }
            } else {
                filter.board_id = board;
                filter.space_id = space;
            }
            filter.column_id = column;
            filter.query = query;
            filter.tag = tag;
            filter.type_id = type_id;
            if archived {
                filter.archived = Some(true);
            }
            if let Some(member_id) = member {
                filter.member_ids.push(member_id);
            }
            if mine {
                let me = client.users().current().await?;
                filter.member_ids.push(me.id);
            }
            let cards = client.cards().list(&filter).await?;
            if json {
                return output::print_json(&cards);
            }
            let mut table = output::table(&["ID", "TITLE", "COLUMN", "TYPE", "ASAP", "UPDATED"]);
            for card in &cards {
                table.add_row(vec![
                    card.id.to_string(),
                    card.title.clone(),
                    card.column
                        .as_ref()
                        .map(|c| c.title.clone())
                        .unwrap_or_else(|| "-".into()),
                    card.card_type
                        .as_ref()
                        .and_then(|t| t.letter.clone())
                        .unwrap_or_else(|| "-".into()),
                    if card.asap.unwrap_or(false) {
                        "!".into()
                    } else {
                        String::new()
                    },
                    card.updated
                        .as_deref()
                        .and_then(|d| d.split('T').next())
                        .unwrap_or("-")
                        .to_string(),
                ]);
            }
            println!("{table}");
            Ok(())
        }
        CardCmd::View { .. }
        | CardCmd::Create { .. }
        | CardCmd::Edit { .. }
        | CardCmd::Move { .. }
        | CardCmd::Archive { .. } => Err(CliError::InvalidArg("not implemented yet".into())),
        CardCmd::Member(_) | CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
    }
}
