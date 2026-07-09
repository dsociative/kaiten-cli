use kaiten_client::{CardFilter, KaitenClient};

use crate::cli::CardCmd;
use crate::config::Defaults;
use crate::error::CliError;
use crate::output;

/// Accepts a numeric card id or a browser URL containing `card/<id>`.
pub fn parse_card_ref(s: &str) -> Result<u64, CliError> {
    if let Ok(id) = s.parse::<u64>() {
        return Ok(id);
    }
    if let Some(pos) = s.find("card/") {
        let digits: String = s[pos + "card/".len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(id) = digits.parse::<u64>() {
            return Ok(id);
        }
    }
    Err(CliError::InvalidArg(format!(
        "invalid card reference `{s}`: expected a numeric id or a card URL"
    )))
}

fn user_display(user: &kaiten_client::User) -> String {
    user.username
        .clone()
        .or_else(|| user.full_name.clone())
        .unwrap_or_else(|| user.id.to_string())
}

fn print_card_details(card: &kaiten_client::Card) {
    println!("#{} {}", card.id, card.title);
    println!();
    let dash = || "-".to_string();
    println!(
        "board:   {}",
        card.board
            .as_ref()
            .map(|b| format!("{} ({})", b.title, b.id))
            .unwrap_or_else(dash)
    );
    println!(
        "column:  {}",
        card.column
            .as_ref()
            .map(|c| format!("{} ({})", c.title, c.id))
            .unwrap_or_else(dash)
    );
    println!(
        "lane:    {}",
        card.lane
            .as_ref()
            .map(|l| format!("{} ({})", l.title, l.id))
            .unwrap_or_else(dash)
    );
    println!(
        "type:    {}",
        card.card_type
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(dash)
    );
    println!(
        "owner:   {}",
        card.owner.as_ref().map(user_display).unwrap_or_else(dash)
    );
    let members = card
        .members
        .iter()
        .map(|m| {
            m.username
                .clone()
                .or_else(|| m.full_name.clone())
                .unwrap_or_else(|| m.id.to_string())
        })
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "members: {}",
        if members.is_empty() { dash() } else { members }
    );
    let tags = card
        .tags
        .iter()
        .map(|t| t.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    println!("tags:    {}", if tags.is_empty() { dash() } else { tags });
    println!(
        "asap:    {}",
        if card.asap.unwrap_or(false) {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "created: {}",
        card.created
            .as_deref()
            .and_then(|d| d.split('T').next())
            .unwrap_or("-")
    );
    println!(
        "updated: {}",
        card.updated
            .as_deref()
            .and_then(|d| d.split('T').next())
            .unwrap_or("-")
    );
    if let Some(description) = &card.description {
        println!();
        println!("Description:");
        println!("{description}");
    }
    if !card.checklists.is_empty() {
        println!();
        println!("Checklists:");
        for checklist in &card.checklists {
            println!("{} ({})", checklist.name, checklist.id);
            for item in &checklist.items {
                let mark = if item.checked.unwrap_or(false) {
                    "x"
                } else {
                    " "
                };
                println!("  [{mark}] {} ({})", item.text, item.id);
            }
        }
    }
    if let Some(properties) = card.properties.as_ref().filter(|p| !p.is_null()) {
        println!();
        println!("Properties:");
        println!(
            "{}",
            serde_json::to_string_pretty(properties).unwrap_or_else(|_| properties.to_string())
        );
    }
}

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
        CardCmd::View { card, comments } => {
            let card_id = parse_card_ref(&card)?;
            let card = client.cards().get(card_id).await?;
            if json {
                if comments {
                    let list = client.comments().list(card_id).await?;
                    return output::print_json(&serde_json::json!({
                        "card": card,
                        "comments": list,
                    }));
                }
                return output::print_json(&card);
            }
            print_card_details(&card);
            if comments {
                let list = client.comments().list(card_id).await?;
                println!();
                println!("Comments:");
                for comment in &list {
                    let author = comment
                        .author
                        .as_ref()
                        .map(user_display)
                        .unwrap_or_else(|| "-".into());
                    let date = comment
                        .created
                        .as_deref()
                        .and_then(|d| d.split('T').next())
                        .unwrap_or("-");
                    println!("{date} {author}:");
                    println!("{}", comment.text);
                }
            }
            Ok(())
        }
        CardCmd::Create { .. }
        | CardCmd::Edit { .. }
        | CardCmd::Move { .. }
        | CardCmd::Archive { .. } => Err(CliError::InvalidArg("not implemented yet".into())),
        CardCmd::Member(_) | CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_id() {
        assert_eq!(parse_card_ref("67089469").unwrap(), 67089469);
    }

    #[test]
    fn parses_browser_url() {
        let url = "https://mycompany.kaiten.ru/space/810671/boards/card/67089469";
        assert_eq!(parse_card_ref(url).unwrap(), 67089469);
    }

    #[test]
    fn parses_url_with_query_suffix() {
        let url = "https://mycompany.kaiten.ru/space/810671/card/67089469?focus=comments";
        assert_eq!(parse_card_ref(url).unwrap(), 67089469);
    }

    #[test]
    fn garbage_is_invalid_arg() {
        let err = parse_card_ref("definitely-not-a-card").unwrap_err();
        assert!(matches!(err, CliError::InvalidArg(_)));
        assert!(err.to_string().contains("invalid card reference"), "{err}");
    }

    #[test]
    fn url_without_digits_is_invalid_arg() {
        let err = parse_card_ref("https://mycompany.kaiten.ru/card/").unwrap_err();
        assert!(matches!(err, CliError::InvalidArg(_)));
    }
}
