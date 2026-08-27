use std::path::Path;

use kaiten_client::{CardFilter, CreateCard, FileRef, KaitenClient, UpdateCard};

use crate::cli::{
    CardChecklistCmd, CardChecklistItemCmd, CardCmd, CardCommentCmd, CardExternalLinkCmd,
    CardFileCmd, CardMemberCmd, CardTagCmd, CardTimeCmd, ViewSection,
};
use crate::config::Defaults;
use crate::download;
use crate::error::CliError;
use crate::output;
use crate::properties;
use crate::urls;

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
    if let Some(properties) = card
        .properties
        .as_ref()
        .filter(|_| !properties::no_properties(&card.properties))
    {
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
        CardCmd::View {
            card,
            comments,
            include,
        } => run_view(client, json, &card, comments, &include).await,
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
        CardCmd::Link {
            card,
            child,
            parent,
            blocks,
            blocked_by,
            reason,
        } => {
            run_link(
                client,
                json,
                &card,
                (child, parent, blocks, blocked_by),
                reason,
            )
            .await
        }
        CardCmd::Unlink {
            card,
            child,
            parent,
            blocks,
            blocked_by,
        } => run_unlink(client, json, &card, (child, parent, blocks, blocked_by)).await,
        CardCmd::Unblock { card } => run_unblock(client, json, &card).await,
        CardCmd::Delete { card, yes } => run_delete(client, json, &card, yes).await,
        CardCmd::Time(cmd) => run_time(client, json, cmd).await,
        CardCmd::Member(cmd) => run_member(client, json, cmd).await,
        CardCmd::Comment(cmd) => run_comment(client, json, cmd).await,
        CardCmd::ExternalLink(cmd) => run_external_link(client, json, cmd).await,
        CardCmd::Checklist(cmd) => run_checklist(client, json, cmd).await,
        CardCmd::Tag(cmd) => run_tag(client, json, cmd).await,
        CardCmd::File(cmd) => run_file(client, json, cmd).await,
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
    let mut filter = CardFilter::default();
    filter.limit = Some(filters.limit);
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
    comments_flag: bool,
    include: &[ViewSection],
) -> Result<(), CliError> {
    if comments_flag {
        eprintln!("warning: --comments is deprecated, use --include comments");
    }
    let with_links = include.contains(&ViewSection::ExternalLinks);
    let with_comments = comments_flag || include.contains(&ViewSection::Comments);
    let card_id = parse_card_ref(card)?;
    let card = client.cards().get(card_id).await?;
    let links = if with_links {
        Some(client.external_links().list(card_id).await?)
    } else {
        None
    };
    let comments = if with_comments {
        Some(client.comments().list(card_id).await?)
    } else {
        None
    };
    if json {
        if links.is_none() && comments.is_none() {
            return output::print_json(&card);
        }
        let mut doc = serde_json::Map::new();
        doc.insert("card".into(), serde_json::json!(card));
        if let Some(links) = &links {
            doc.insert("external_links".into(), serde_json::json!(links));
        }
        if let Some(comments) = &comments {
            doc.insert("comments".into(), serde_json::json!(comments));
        }
        return output::print_json(&doc);
    }
    print_card_details(&card);
    if let Some(links) = &links {
        print_links_section(links);
    }
    if let Some(comments) = &comments {
        print_comments_section(comments);
    }
    Ok(())
}

fn print_links_section(links: &[kaiten_client::ExternalLink]) {
    println!();
    println!("External links:");
    for link in links {
        match link.description.as_deref().filter(|d| !d.is_empty()) {
            Some(description) => println!("  {} {} - {description}", link.id, link.url),
            None => println!("  {} {}", link.id, link.url),
        }
    }
}

fn print_comments_section(comments: &[kaiten_client::Comment]) {
    println!();
    println!("Comments:");
    for comment in comments {
        let author = comment
            .author
            .as_ref()
            .map_or_else(|| "-".into(), output::user_label);
        let date = date_cell(comment.created.as_deref());
        println!("{date} {author}:");
        println!("{}", comment.text);
    }
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

/// `--properties-json` must be a JSON OBJECT keyed as id_{property_id}; a JSON
/// string holding such an object is accepted too (see `crate::properties`).
fn parse_properties_json(raw: Option<String>) -> Result<Option<serde_json::Value>, CliError> {
    let Some(raw) = raw else { return Ok(None) };
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| CliError::InvalidArg(format!("--properties-json is not valid JSON: {e}")))?;
    properties::coerce_object(value)
        .map(Some)
        .map_err(|msg| CliError::InvalidArg(format!("--properties-json {msg}")))
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
    let mut req = CreateCard::new(board_id, args.title);
    req.column_id = args.column;
    req.lane_id = args.lane;
    req.description = args.description;
    req.type_id = args.type_id;
    req.asap = if args.asap { Some(true) } else { None };
    req.properties = parse_properties_json(args.properties_json)?;
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
    let mut req = UpdateCard::default();
    req.title = args.title;
    req.description = args.description;
    req.type_id = args.type_id;
    req.asap = args.asap;
    req.properties = parse_properties_json(args.properties_json)?;
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
    let mut req = UpdateCard::default();
    req.column_id = Some(column);
    req.lane_id = lane;
    req.board_id = board;
    let card = client.cards().update(card_id, &req).await?;
    if json {
        return output::print_json(&card);
    }
    print_card_kv(&card);
    Ok(())
}

async fn run_archive(client: &KaitenClient, json: bool, card: &str) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let mut req = UpdateCard::default();
    req.condition = Some(2);
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
        CardMemberCmd::Responsible { card, user, unset } => {
            let card_id = parse_card_ref(&card)?;
            let user_id = resolve_user(client, &user).await?;
            let member = client
                .members()
                .update_role(card_id, user_id, !unset)
                .await?;
            if json {
                return output::print_json(&member);
            }
            if unset {
                println!("user {user_id} is a regular member of card {card_id} again");
            } else {
                println!("user {user_id} is now responsible for card {card_id}");
            }
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
        CardCommentCmd::Edit {
            card,
            comment_id,
            body,
        } => {
            let card_id = parse_card_ref(&card)?;
            let comment = client.comments().update(card_id, comment_id, &body).await?;
            if json {
                return output::print_json(&comment);
            }
            println!("updated comment {} on card {card_id}", comment.id);
            Ok(())
        }
        CardCommentCmd::Rm { card, comment_id } => {
            let card_id = parse_card_ref(&card)?;
            client.comments().remove(card_id, comment_id).await?;
            if json {
                return output::print_json(&serde_json::json!({ "removed": true }));
            }
            println!("removed comment {comment_id} from card {card_id}");
            Ok(())
        }
    }
}

async fn run_external_link(
    client: &KaitenClient,
    json: bool,
    cmd: CardExternalLinkCmd,
) -> Result<(), CliError> {
    match cmd {
        CardExternalLinkCmd::Add {
            card,
            url,
            description,
        } => {
            let card_id = parse_card_ref(&card)?;
            let url = link_url(&url)?;
            let link = client
                .external_links()
                .add(card_id, &url, description.as_deref())
                .await?;
            if json {
                return output::print_json(&link);
            }
            println!("{}", link.id);
            Ok(())
        }
        CardExternalLinkCmd::List { card } => {
            let card_id = parse_card_ref(&card)?;
            let links = client.external_links().list(card_id).await?;
            if json {
                return output::print_json(&links);
            }
            let mut table = output::table(&["ID", "URL", "DESCRIPTION", "CREATED"]);
            for link in &links {
                table.add_row(vec![
                    link.id.to_string(),
                    truncate_text(&link.url, 70),
                    truncate_text(
                        link.description
                            .as_deref()
                            .filter(|d| !d.is_empty())
                            .unwrap_or("-"),
                        40,
                    ),
                    date_cell(link.created.as_deref()),
                ]);
            }
            println!("{table}");
            Ok(())
        }
        CardExternalLinkCmd::Edit {
            card,
            link_id,
            url,
            description,
        } => {
            if url.is_none() && description.is_none() {
                return Err(CliError::InvalidArg(
                    "nothing to change: pass --url and/or --description".into(),
                ));
            }
            let card_id = parse_card_ref(&card)?;
            let url = url.as_deref().map(link_url).transpose()?;
            let link = client
                .external_links()
                .update(card_id, link_id, url.as_deref(), description.as_deref())
                .await?;
            if json {
                return output::print_json(&link);
            }
            println!("updated external link {} on card {card_id}", link.id);
            Ok(())
        }
        CardExternalLinkCmd::Rm { card, link_id } => {
            let card_id = parse_card_ref(&card)?;
            client.external_links().remove(card_id, link_id).await?;
            if json {
                return output::print_json(&serde_json::json!({ "removed": true }));
            }
            println!("removed external link {link_id} from card {card_id}");
            Ok(())
        }
    }
}

/// `--url` must be an absolute http(s) URL; the value is never echoed.
fn link_url(raw: &str) -> Result<String, CliError> {
    urls::absolute_http_url(raw).map_err(|e| CliError::InvalidArg(format!("--url {e}")))
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

/// (child, parent, blocks, blocked_by) — exactly one must be set
/// (clap's arg group guarantees at most one).
type LinkFlags = (Option<u64>, Option<u64>, Option<u64>, Option<u64>);

async fn run_link(
    client: &KaitenClient,
    json: bool,
    card: &str,
    flags: LinkFlags,
    reason: Option<String>,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let (child, parent, blocks, blocked_by) = flags;
    let described = match (child, parent, blocks, blocked_by) {
        (Some(target), None, None, None) => {
            client.links().add_child(card_id, target).await?;
            format!("card {target} is now a child of {card_id}")
        }
        (None, Some(target), None, None) => {
            client.links().add_child(target, card_id).await?;
            format!("card {target} is now a parent of {card_id}")
        }
        (None, None, Some(target), None) => {
            client
                .links()
                .add_blocker(target, Some(card_id), reason.as_deref())
                .await?;
            format!("card {card_id} now blocks {target}")
        }
        (None, None, None, Some(target)) => {
            client
                .links()
                .add_blocker(card_id, Some(target), reason.as_deref())
                .await?;
            format!("card {card_id} is now blocked by {target}")
        }
        _ => {
            return Err(CliError::InvalidArg(
                "pass exactly one of --child/--parent/--blocks/--blocked-by".into(),
            ));
        }
    };
    if json {
        return output::print_json(&serde_json::json!({ "linked": true }));
    }
    println!("{described}");
    Ok(())
}

async fn run_unlink(
    client: &KaitenClient,
    json: bool,
    card: &str,
    flags: LinkFlags,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let (child, parent, blocks, blocked_by) = flags;
    match (child, parent, blocks, blocked_by) {
        (Some(target), None, None, None) => {
            client.links().remove_child(card_id, target).await?;
        }
        (None, Some(target), None, None) => {
            client.links().remove_child(target, card_id).await?;
        }
        (None, None, Some(target), None) | (None, None, None, Some(target)) => {
            // the blocker entry lives on the BLOCKED card
            let (blocked_id, blocker_card_id) = if blocks.is_some() {
                (target, card_id)
            } else {
                (card_id, target)
            };
            let blocked_card = client.cards().get(blocked_id).await?;
            let Some(blocker) = blocked_card
                .blockers
                .iter()
                .find(|b| b.blocker_card_id == Some(blocker_card_id))
            else {
                return Err(CliError::InvalidArg(format!(
                    "card {blocked_id} has no blocker with card {blocker_card_id}"
                )));
            };
            client
                .links()
                .remove_blocker(blocked_id, blocker.id)
                .await?;
        }
        _ => {
            return Err(CliError::InvalidArg(
                "pass exactly one of --child/--parent/--blocks/--blocked-by".into(),
            ));
        }
    }
    if json {
        return output::print_json(&serde_json::json!({ "unlinked": true }));
    }
    println!("unlinked");
    Ok(())
}

async fn run_unblock(client: &KaitenClient, json: bool, card: &str) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let mut req = UpdateCard::default();
    req.blocked = Some(false);
    let card = client.cards().update(card_id, &req).await?;
    if json {
        return output::print_json(&card);
    }
    println!("released all blocks on card {}", card.id);
    Ok(())
}

async fn run_file(client: &KaitenClient, json: bool, cmd: CardFileCmd) -> Result<(), CliError> {
    match cmd {
        CardFileCmd::Add { card, path } => {
            let card_id = parse_card_ref(&card)?;
            let file = client.files().attach(card_id, &path).await?;
            if json {
                return output::print_json(&file);
            }
            println!(
                "attached {} ({} bytes) to card {card_id}\nurl (public!): {}",
                file.name,
                file.size.unwrap_or(0),
                file.url.as_deref().unwrap_or("-")
            );
            Ok(())
        }
        CardFileCmd::Rm { card, file_id } => {
            let card_id = parse_card_ref(&card)?;
            client.files().detach(card_id, file_id).await?;
            if json {
                return output::print_json(&serde_json::json!({ "detached": true }));
            }
            println!("detached file {file_id} from card {card_id}");
            Ok(())
        }
        CardFileCmd::List { card } => run_file_list(client, json, &card).await,
        CardFileCmd::Get {
            card,
            file,
            output,
            force,
        } => run_file_get(client, json, &card, &file, output.as_deref(), force).await,
    }
}

async fn run_file_list(client: &KaitenClient, json: bool, card: &str) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let files = client.files().list(card_id).await?;
    if json {
        return output::print_json(&files);
    }
    if files.is_empty() {
        println!("no files on card {card_id}");
        return Ok(());
    }
    let mut table = output::table(&["ID", "NAME", "SIZE", "MIME TYPE", "CREATED"]);
    for f in &files {
        table.add_row(vec![
            FileRef::from(f).to_string(),
            truncate_text(&f.name, 50),
            f.size.map_or_else(|| "-".to_string(), |s| s.to_string()),
            f.mime_type.clone().unwrap_or_else(|| "-".to_string()),
            date_cell(f.created.as_deref()),
        ]);
    }
    println!("{table}");
    Ok(())
}

async fn run_file_get(
    client: &KaitenClient,
    json: bool,
    card: &str,
    file: &str,
    to: Option<&Path>,
    force: bool,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let Ok(file_ref) = file.parse::<FileRef>();
    let files = client.files().list(card_id).await?;
    let file = download::find_file(card_id, &files, &file_ref)?;
    let name = download::safe_file_name(file);
    // "" as the default directory: the current directory, printed absolute by `save`
    let target = download::target_path(to, Path::new(""), &name)?;
    download::ensure_writable(&target, force, "--force")?;
    let saved = download::save(client, file, &target).await?;
    if json {
        return output::print_json(&saved);
    }
    println!(
        "saved {} ({} bytes) to {}",
        saved.name, saved.size, saved.path
    );
    Ok(())
}

async fn run_delete(
    client: &KaitenClient,
    json: bool,
    card: &str,
    yes: bool,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let target = client.cards().get(card_id).await?;
    if !yes {
        eprint!(
            "PERMANENTLY delete card {card_id} \"{}\"? Type the card id to confirm: ",
            target.title
        );
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(CliError::Io)?;
        if answer.trim() != card_id.to_string() {
            return Err(CliError::InvalidArg(
                "confirmation did not match the card id; nothing deleted".into(),
            ));
        }
    }
    client.cards().delete(card_id).await?;
    if json {
        return output::print_json(&serde_json::json!({ "deleted": true, "card_id": card_id }));
    }
    println!("deleted card {card_id}");
    Ok(())
}

async fn run_time(client: &KaitenClient, json: bool, cmd: CardTimeCmd) -> Result<(), CliError> {
    match cmd {
        CardTimeCmd::Add {
            card,
            minutes,
            date,
            comment,
            role,
        } => {
            let card_id = parse_card_ref(&card)?;
            let role_id = if let Some(id) = role {
                id
            } else {
                let roles = client.users().roles().await?;
                roles
                    .first()
                    .ok_or_else(|| {
                        CliError::InvalidArg(
                            "no user roles in the company; pass --role explicitly".into(),
                        )
                    })?
                    .id
            };
            let log = client
                .time_logs()
                .add(card_id, minutes, &date, role_id, comment.as_deref())
                .await?;
            if json {
                return output::print_json(&log);
            }
            println!("logged {} min on card {card_id} ({})", log.time_spent, date);
            Ok(())
        }
        CardTimeCmd::List { card } => {
            let card_id = parse_card_ref(&card)?;
            let logs = client.time_logs().list(card_id).await?;
            if json {
                return output::print_json(&logs);
            }
            let mut table = output::table(&["ID", "MINUTES", "DATE", "COMMENT"]);
            for log in &logs {
                table.add_row(vec![
                    log.id.to_string(),
                    log.time_spent.to_string(),
                    date_cell(log.for_date.as_deref()),
                    log.comment.clone().unwrap_or_default(),
                ]);
            }
            println!("{table}");
            Ok(())
        }
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
