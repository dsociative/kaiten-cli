use kaiten_client::{CardFilter, CreateCard, KaitenClient, UpdateCard};

use crate::cli::{
    CardChecklistCmd, CardChecklistItemCmd, CardCmd, CardCommentCmd, CardMemberCmd, CardTagCmd,
};
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
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(id) = digits.parse::<u64>() {
            return Ok(id);
        }
    }
    Err(CliError::InvalidArg(format!(
        "invalid card reference `{s}`: expected a numeric id or a card URL"
    )))
}

fn print_card_details(card: &kaiten_client::Card) {
    println!("#{} {}", card.id, card.title);
    println!();
    let dash = || "-".to_string();
    println!(
        "board:   {}",
        card.board
            .as_ref()
            .map_or_else(dash, |b| format!("{} ({})", b.title, b.id))
    );
    println!(
        "column:  {}",
        card.column
            .as_ref()
            .map_or_else(dash, |c| format!("{} ({})", c.title, c.id))
    );
    println!(
        "lane:    {}",
        card.lane
            .as_ref()
            .map_or_else(dash, |l| format!("{} ({})", l.title, l.id))
    );
    println!(
        "type:    {}",
        card.card_type
            .as_ref()
            .map_or_else(dash, |t| t.name.clone())
    );
    println!(
        "owner:   {}",
        card.owner.as_ref().map_or_else(dash, output::user_label)
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
    println!("created: {}", date_cell(card.created.as_deref()));
    println!("updated: {}", date_cell(card.updated.as_deref()));
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

fn print_card_kv(card: &kaiten_client::Card) {
    let dash = || "-".to_string();
    let mut table = output::table(&["FIELD", "VALUE"]);
    table.add_row(vec!["id".to_string(), card.id.to_string()]);
    table.add_row(vec!["title".to_string(), card.title.clone()]);
    table.add_row(vec![
        "board".to_string(),
        card.board_id.map_or_else(dash, |v| v.to_string()),
    ]);
    table.add_row(vec![
        "column".to_string(),
        card.column_id.map_or_else(dash, |v| v.to_string()),
    ]);
    table.add_row(vec![
        "lane".to_string(),
        card.lane_id.map_or_else(dash, |v| v.to_string()),
    ]);
    table.add_row(vec![
        "type".to_string(),
        card.type_id.map_or_else(dash, |v| v.to_string()),
    ]);
    table.add_row(vec![
        "asap".to_string(),
        card.asap.map_or_else(dash, |v| v.to_string()),
    ]);
    table.add_row(vec![
        "condition".to_string(),
        card.condition.map_or_else(dash, |v| v.to_string()),
    ]);
    table.add_row(vec![
        "updated".to_string(),
        date_cell(card.updated.as_deref()),
    ]);
    println!("{table}");
}

// Pure dispatcher: the length comes from destructuring clap variants
// field-by-field, not from logic.
#[allow(clippy::too_many_lines)]
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
            states,
            updated_after,
            created_after,
            sort,
            desc,
            limit,
            offset,
        } => {
            run_list(
                client,
                defaults,
                json,
                CardListFilters {
                    space,
                    board,
                    column,
                    mine,
                    member,
                    query,
                    tag,
                    type_id,
                    archived,
                    states,
                    updated_after,
                    created_after,
                    sort,
                    desc,
                    limit,
                    offset,
                },
            )
            .await
        }
        CardCmd::View { card, comments } => run_view(client, json, &card, comments).await,
        CardCmd::Create {
            title,
            board,
            column,
            lane,
            description,
            type_id,
            asap,
            properties_json,
        } => {
            run_create(
                client,
                defaults,
                json,
                CardCreateArgs {
                    title,
                    board,
                    column,
                    lane,
                    description,
                    type_id,
                    asap,
                    properties_json,
                },
            )
            .await
        }
        CardCmd::Edit {
            card,
            title,
            description,
            type_id,
            asap,
            properties_json,
        } => {
            run_edit(
                client,
                json,
                &card,
                CardEditArgs {
                    title,
                    description,
                    type_id,
                    asap,
                    properties_json,
                },
            )
            .await
        }
        CardCmd::Move {
            card,
            column,
            lane,
            board,
        } => run_move(client, json, &card, column, lane, board).await,
        CardCmd::Archive { card } => run_archive(client, json, &card).await,
        CardCmd::Member(cmd) => run_member(client, json, cmd).await,
        CardCmd::Comment(cmd) => run_comment(client, json, cmd).await,
        CardCmd::Checklist(cmd) => run_checklist(client, json, cmd).await,
        CardCmd::Tag(cmd) => run_tag(client, json, cmd).await,
    }
}

struct CardListFilters {
    space: Option<u64>,
    board: Option<u64>,
    column: Option<u64>,
    mine: bool,
    member: Option<u64>,
    query: Option<String>,
    tag: Option<String>,
    type_id: Option<u64>,
    archived: bool,
    states: Vec<crate::cli::CardState>,
    updated_after: Option<String>,
    created_after: Option<String>,
    sort: Option<String>,
    desc: bool,
    limit: u32,
    offset: Option<u32>,
}

async fn run_list(
    client: &KaitenClient,
    defaults: &Defaults,
    json: bool,
    filters: CardListFilters,
) -> Result<(), CliError> {
    let mut filter = CardFilter {
        limit: Some(filters.limit),
        ..Default::default()
    };
    if filters.board.is_none() && filters.space.is_none() {
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
        filter.board_id = filters.board;
        filter.space_id = filters.space;
    }
    filter.column_id = filters.column;
    filter.query = filters.query;
    filter.tag = filters.tag;
    filter.type_id = filters.type_id;
    filter.archived = Some(filters.archived);
    filter.states = filters
        .states
        .iter()
        .map(|s| crate::cli::CardState::as_u8(*s))
        .collect();
    filter.updated_after = filters.updated_after;
    filter.created_after = filters.created_after;
    filter.order_by = filters.sort;
    if filter.order_by.is_some() {
        filter.order_direction = Some(if filters.desc { "desc" } else { "asc" }.to_string());
    }
    filter.offset = filters.offset;
    if let Some(member_id) = filters.member {
        filter.member_ids.push(member_id);
    }
    if filters.mine {
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
                .map_or_else(|| "-".into(), |c| c.title.clone()),
            card.card_type
                .as_ref()
                .and_then(|t| t.letter.clone())
                .unwrap_or_else(|| "-".into()),
            if card.asap.unwrap_or(false) {
                "!".into()
            } else {
                String::new()
            },
            date_cell(card.updated.as_deref()),
        ]);
    }
    println!("{table}");
    Ok(())
}

async fn run_view(
    client: &KaitenClient,
    json: bool,
    card: &str,
    comments: bool,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
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
                .map_or_else(|| "-".into(), output::user_label);
            let date = date_cell(comment.created.as_deref());
            println!("{date} {author}:");
            println!("{}", comment.text);
        }
    }
    Ok(())
}

struct CardCreateArgs {
    title: String,
    board: Option<u64>,
    column: Option<u64>,
    lane: Option<u64>,
    description: Option<String>,
    type_id: Option<u64>,
    asap: bool,
    properties_json: Option<String>,
}

struct CardEditArgs {
    title: Option<String>,
    description: Option<String>,
    type_id: Option<u64>,
    asap: Option<bool>,
    properties_json: Option<String>,
}

/// `--properties-json` must be a JSON OBJECT keyed as id_{property_id}.
fn parse_properties_json(raw: Option<String>) -> Result<Option<serde_json::Value>, CliError> {
    let Some(raw) = raw else { return Ok(None) };
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| CliError::InvalidArg(format!("--properties-json is not valid JSON: {e}")))?;
    if !value.is_object() {
        return Err(CliError::InvalidArg(
            "--properties-json must be a JSON object like '{\"id_612634\": [18929916]}'".into(),
        ));
    }
    Ok(Some(value))
}

async fn run_create(
    client: &KaitenClient,
    defaults: &Defaults,
    json: bool,
    args: CardCreateArgs,
) -> Result<(), CliError> {
    let board_id = args.board.or(defaults.board).ok_or_else(|| {
        CliError::InvalidArg("specify --board or set defaults.board in config".into())
    })?;
    let req = CreateCard {
        board_id,
        title: args.title,
        column_id: args.column,
        lane_id: args.lane,
        description: args.description,
        type_id: args.type_id,
        asap: if args.asap { Some(true) } else { None },
        properties: parse_properties_json(args.properties_json)?,
    };
    let card = client.cards().create(&req).await?;
    if json {
        return output::print_json(&card);
    }
    print_card_kv(&card);
    Ok(())
}

async fn run_edit(
    client: &KaitenClient,
    json: bool,
    card: &str,
    args: CardEditArgs,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    if args.title.is_none()
        && args.description.is_none()
        && args.type_id.is_none()
        && args.asap.is_none()
        && args.properties_json.is_none()
    {
        return Err(CliError::InvalidArg(
            "nothing to edit: pass --title/--description/--type/--asap/--properties-json".into(),
        ));
    }
    let req = UpdateCard {
        title: args.title,
        description: args.description,
        type_id: args.type_id,
        asap: args.asap,
        properties: parse_properties_json(args.properties_json)?,
        ..Default::default()
    };
    let card = client.cards().update(card_id, &req).await?;
    if json {
        return output::print_json(&card);
    }
    print_card_kv(&card);
    Ok(())
}

async fn run_move(
    client: &KaitenClient,
    json: bool,
    card: &str,
    column: u64,
    lane: Option<u64>,
    board: Option<u64>,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let req = UpdateCard {
        column_id: Some(column),
        lane_id: lane,
        board_id: board,
        ..Default::default()
    };
    let card = client.cards().update(card_id, &req).await?;
    if json {
        return output::print_json(&card);
    }
    print_card_kv(&card);
    Ok(())
}

async fn run_archive(client: &KaitenClient, json: bool, card: &str) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let req = UpdateCard {
        condition: Some(2),
        ..Default::default()
    };
    let card = client.cards().update(card_id, &req).await?;
    if json {
        return output::print_json(&card);
    }
    print_card_kv(&card);
    Ok(())
}

async fn run_member(client: &KaitenClient, json: bool, cmd: CardMemberCmd) -> Result<(), CliError> {
    match cmd {
        CardMemberCmd::Add { card, user } => {
            let card_id = parse_card_ref(&card)?;
            let user_id = resolve_user(client, &user).await?;
            let member = client.members().add(card_id, user_id).await?;
            if json {
                return output::print_json(&member);
            }
            println!("added user {user_id} to card {card_id}");
            Ok(())
        }
        CardMemberCmd::Remove { card, user } => {
            let card_id = parse_card_ref(&card)?;
            let user_id = resolve_user(client, &user).await?;
            client.members().remove(card_id, user_id).await?;
            if json {
                return output::print_json(&serde_json::json!({
                    "removed": true,
                    "user_id": user_id,
                }));
            }
            println!("removed user {user_id} from card {card_id}");
            Ok(())
        }
    }
}

async fn run_comment(
    client: &KaitenClient,
    json: bool,
    cmd: CardCommentCmd,
) -> Result<(), CliError> {
    match cmd {
        CardCommentCmd::Add { card, body } => {
            let card_id = parse_card_ref(&card)?;
            let comment = client.comments().add(card_id, &body).await?;
            if json {
                return output::print_json(&comment);
            }
            println!("{}", comment.id);
            Ok(())
        }
        CardCommentCmd::List { card } => {
            let card_id = parse_card_ref(&card)?;
            let comments = client.comments().list(card_id).await?;
            if json {
                return output::print_json(&comments);
            }
            let mut table = output::table(&["ID", "AUTHOR", "CREATED", "TEXT"]);
            for comment in &comments {
                let author = comment
                    .author
                    .as_ref()
                    .and_then(|a| a.username.as_deref())
                    .unwrap_or("-")
                    .to_string();
                table.add_row(vec![
                    comment.id.to_string(),
                    author,
                    date_cell(comment.created.as_deref()),
                    truncate_text(&comment.text, 60),
                ]);
            }
            println!("{table}");
            Ok(())
        }
    }
}

async fn run_checklist(
    client: &KaitenClient,
    json: bool,
    cmd: CardChecklistCmd,
) -> Result<(), CliError> {
    match cmd {
        CardChecklistCmd::List { card } => {
            let card_id = parse_card_ref(&card)?;
            let card = client.cards().get(card_id).await?;
            if json {
                return output::print_json(&card.checklists);
            }
            if card.checklists.is_empty() {
                println!("no checklists on card {card_id}");
                return Ok(());
            }
            for checklist in &card.checklists {
                println!("{} ({})", checklist.name, checklist.id);
                for item in &checklist.items {
                    let mark = if item.checked.unwrap_or(false) {
                        "x"
                    } else {
                        " "
                    };
                    println!("  [{mark}] {} {}", item.id, item.text);
                }
            }
            Ok(())
        }
        CardChecklistCmd::Add { card, name } => {
            let card_id = parse_card_ref(&card)?;
            let checklist = client.checklists().add(card_id, &name).await?;
            if json {
                return output::print_json(&checklist);
            }
            println!("created checklist {}", checklist.id);
            Ok(())
        }
        CardChecklistCmd::Item(cmd) => match cmd {
            CardChecklistItemCmd::Add {
                card,
                checklist_id,
                text,
            } => {
                let card_id = parse_card_ref(&card)?;
                let item = client
                    .checklists()
                    .add_item(card_id, checklist_id, &text)
                    .await?;
                if json {
                    return output::print_json(&item);
                }
                println!("created item {}", item.id);
                Ok(())
            }
            CardChecklistItemCmd::Check {
                card,
                checklist_id,
                item_id,
            } => set_item_checked(client, json, &card, checklist_id, item_id, true).await,
            CardChecklistItemCmd::Uncheck {
                card,
                checklist_id,
                item_id,
            } => set_item_checked(client, json, &card, checklist_id, item_id, false).await,
        },
    }
}

async fn run_tag(client: &KaitenClient, json: bool, cmd: CardTagCmd) -> Result<(), CliError> {
    match cmd {
        CardTagCmd::Add { card, name } => {
            let card_id = parse_card_ref(&card)?;
            let tag = client.tags().add_to_card(card_id, &name).await?;
            if json {
                return output::print_json(&tag);
            }
            println!("added tag {} ({}) to card {card_id}", tag.name, tag.id);
            Ok(())
        }
        CardTagCmd::Remove { card, name } => {
            let card_id = parse_card_ref(&card)?;
            let card = client.cards().get(card_id).await?;
            let Some(card_tag) = card.tags.iter().find(|t| t.name == name) else {
                let existing = if card.tags.is_empty() {
                    "(none)".to_string()
                } else {
                    card.tags
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                return Err(CliError::InvalidArg(format!(
                    "card {card_id} has no tag `{name}`; existing tags: {existing}"
                )));
            };
            let tag_id = card_tag.tag_id.unwrap_or(card_tag.id);
            client.tags().remove_from_card(card_id, tag_id).await?;
            if json {
                return output::print_json(&serde_json::json!({
                    "removed": true,
                    "tag": name,
                }));
            }
            println!("removed tag {name} from card {card_id}");
            Ok(())
        }
    }
}

async fn set_item_checked(
    client: &KaitenClient,
    json: bool,
    card: &str,
    checklist_id: u64,
    item_id: u64,
    checked: bool,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let item = client
        .checklists()
        .set_item_checked(card_id, checklist_id, item_id, checked)
        .await?;
    if json {
        return crate::output::print_json(&item);
    }
    println!(
        "item {} {}",
        item.id,
        if checked { "checked" } else { "unchecked" }
    );
    Ok(())
}

/// Resolve a `<user>` CLI argument into a user id.
/// Numeric string -> id as is; contains `@` -> exact email match via GET /users.
async fn resolve_user(client: &KaitenClient, user: &str) -> Result<u64, CliError> {
    if let Ok(id) = user.parse::<u64>() {
        return Ok(id);
    }
    if user.contains('@') {
        let users = client.users().list().await?;
        return users
            .iter()
            .find(|u| u.email.as_deref() == Some(user))
            .map(|u| u.id)
            .ok_or_else(|| CliError::InvalidArg(format!("no user with email `{user}`")));
    }
    Err(CliError::InvalidArg(format!(
        "invalid user `{user}`: expected numeric id or email"
    )))
}

/// Truncate to `max` chars, appending `…` when the text was longer.
fn truncate_text(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

/// ISO datetime -> date part before 'T'; None -> "-".
fn date_cell(value: Option<&str>) -> String {
    match value {
        Some(s) => s.split('T').next().unwrap_or(s).to_string(),
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_id() {
        assert_eq!(parse_card_ref("67089469").unwrap(), 67_089_469);
    }

    #[test]
    fn parses_browser_url() {
        let url = "https://mycompany.kaiten.ru/space/810671/boards/card/67089469";
        assert_eq!(parse_card_ref(url).unwrap(), 67_089_469);
    }

    #[test]
    fn parses_url_with_query_suffix() {
        let url = "https://mycompany.kaiten.ru/space/810671/card/67089469?focus=comments";
        assert_eq!(parse_card_ref(url).unwrap(), 67_089_469);
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
