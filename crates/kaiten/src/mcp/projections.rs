//! Compact projections of client models for MCP tool responses.
//!
//! Tool output is agent context: every byte counts. Projections keep the
//! load-bearing fields, drop nulls/empties via `skip_serializing_if` and
//! flatten nested objects to names and ids. Raw JSON stays available through
//! the CLI (`kaiten card view --json`, `kaiten api`).

use kaiten_client::{Blocker, Card, CardFile, CardMember, Checklist, ChecklistItem, Comment};

#[allow(clippy::trivially_copy_pass_by_ref)] // signature dictated by serde
fn is_false(v: &bool) -> bool {
    !*v
}

#[allow(clippy::trivially_copy_pass_by_ref)] // signature dictated by serde
fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// Kaiten sends `properties: null` for cards that never had custom
/// properties and `properties: {}` after the last one is cleared —
/// both are noise in a projection.
#[allow(clippy::ref_option)] // signature dictated by serde's skip_serializing_if
fn no_properties(v: &Option<serde_json::Value>) -> bool {
    match v {
        None => true,
        Some(v) => v.is_null() || v.as_object().is_some_and(serde_json::Map::is_empty),
    }
}

/// One card in a list response: `list_cards`, `poll_updates`.
#[derive(Debug, serde::Serialize)]
pub struct CardSummary {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_id: Option<u64>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u8>,
    #[serde(skip_serializing_if = "is_false")]
    pub archived: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub asap: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(skip_serializing_if = "is_zero")]
    pub comments_total: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub children_count: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub parents_count: u32,
}

impl From<&Card> for CardSummary {
    fn from(card: &Card) -> Self {
        Self {
            id: card.id,
            key: card.key.clone(),
            title: card.title.clone(),
            column: card.column.as_ref().map(|c| c.title.clone()),
            board_id: card.board_id,
            type_name: card.card_type.as_ref().map(|t| t.name.clone()),
            state: card.state,
            archived: card.archived.unwrap_or(false),
            asap: card.asap.unwrap_or(false),
            blocked: card.blocked.unwrap_or(false),
            due_date: card.due_date.clone(),
            updated: card.updated.clone(),
            comments_total: card.comments_total.unwrap_or(0),
            children_count: card.children_count.unwrap_or(0),
            parents_count: card.parents_count.unwrap_or(0),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct MemberView {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub responsible: bool,
}

impl From<&CardMember> for MemberView {
    fn from(m: &CardMember) -> Self {
        Self {
            id: m.user_id.unwrap_or(m.id),
            name: m.username.clone().or_else(|| m.full_name.clone()),
            responsible: m.member_type == Some(2),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LinkedCardView {
    pub id: u64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u8>,
    #[serde(skip_serializing_if = "is_false")]
    pub archived: bool,
}

impl From<&Card> for LinkedCardView {
    fn from(card: &Card) -> Self {
        Self {
            id: card.id,
            title: card.title.clone(),
            state: card.state,
            archived: card.archived.unwrap_or(false),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct BlockerView {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocker_card_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocker_card_title: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub released: bool,
}

impl From<&Blocker> for BlockerView {
    fn from(b: &Blocker) -> Self {
        Self {
            id: b.id,
            reason: b.reason.clone(),
            blocker_card_id: b.blocker_card_id,
            blocker_card_title: b.blocker_card_title.clone(),
            released: b.released.unwrap_or(false),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct FileView {
    pub id: u64,
    pub name: String,
    /// Public (unguessable) link — served without authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl From<&CardFile> for FileView {
    fn from(f: &CardFile) -> Self {
        Self {
            id: f.id,
            name: f.name.clone(),
            url: f.url.clone(),
            size: f.size,
            mime_type: f.mime_type.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ChecklistItemView {
    pub id: u64,
    pub text: String,
    #[serde(skip_serializing_if = "is_false")]
    pub checked: bool,
}

impl From<&ChecklistItem> for ChecklistItemView {
    fn from(i: &ChecklistItem) -> Self {
        Self {
            id: i.id,
            text: i.text.clone(),
            checked: i.checked.unwrap_or(false),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ChecklistView {
    pub id: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ChecklistItemView>,
}

impl From<&Checklist> for ChecklistView {
    fn from(c: &Checklist) -> Self {
        Self {
            id: c.id,
            name: c.name.clone(),
            items: c.items.iter().map(ChecklistItemView::from).collect(),
        }
    }
}

/// Full card for `get_card`: summary + description, people, checklists,
/// custom properties, links, blockers and files.
#[derive(Debug, serde::Serialize)]
pub struct CardDetail {
    #[serde(flatten)]
    pub summary: CardSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<MemberView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub checklists: Vec<ChecklistView>,
    #[serde(skip_serializing_if = "no_properties")]
    pub properties: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<LinkedCardView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<LinkedCardView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<BlockerView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileView>,
}

impl From<&Card> for CardDetail {
    fn from(card: &Card) -> Self {
        Self {
            summary: CardSummary::from(card),
            column_id: card.column_id,
            lane_id: card.lane_id,
            description: card.description.clone(),
            owner: card
                .owner
                .as_ref()
                .and_then(|u| u.username.clone().or_else(|| u.full_name.clone())),
            members: card.members.iter().map(MemberView::from).collect(),
            tags: card.tags.iter().map(|t| t.name.clone()).collect(),
            checklists: card.checklists.iter().map(ChecklistView::from).collect(),
            properties: card.properties.clone(),
            children: card.children.iter().map(LinkedCardView::from).collect(),
            parents: card.parents.iter().map(LinkedCardView::from).collect(),
            blockers: card.blockers.iter().map(BlockerView::from).collect(),
            files: card.files.iter().map(FileView::from).collect(),
        }
    }
}

/// Response of card mutations (`create_card`, `update_card`, `move_card`):
/// enough to confirm the result and hand the user a link — details on demand
/// via `get_card`.
#[derive(Debug, serde::Serialize)]
pub struct MutationResult {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_id: Option<u64>,
}

impl MutationResult {
    pub fn new(card: &Card, web_base: &str) -> Self {
        Self {
            id: card.id,
            key: card.key.clone(),
            url: format!("{web_base}/{}", card.id),
            title: card.title.clone(),
            column: card.column.as_ref().map(|c| c.title.clone()),
            board_id: card.board_id,
        }
    }
}

/// Response of `add_comment`: the text is the caller's own input — echoing
/// it back would only burn context.
#[derive(Debug, serde::Serialize)]
pub struct CommentResult {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

impl From<&Comment> for CommentResult {
    fn from(c: &Comment) -> Self {
        Self {
            id: c.id,
            created: c.created.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CommentView {
    pub id: u64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub edited: bool,
}

impl From<&Comment> for CommentView {
    fn from(c: &Comment) -> Self {
        Self {
            id: c.id,
            text: c.text.clone(),
            author: c
                .author
                .as_ref()
                .and_then(|u| u.username.clone().or_else(|| u.full_name.clone())),
            created: c.created.clone(),
            edited: c.edited.unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_card() -> Card {
        serde_json::from_str(r#"{"id": 1, "title": "bare"}"#).unwrap()
    }

    #[test]
    fn summary_of_minimal_card_serializes_only_id_and_title() {
        let summary = CardSummary::from(&minimal_card());
        let json = serde_json::to_value(&summary).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, vec!["id", "title"], "no null/false/zero noise");
    }

    #[test]
    fn detail_of_minimal_card_has_no_empty_collections() {
        let detail = CardDetail::from(&minimal_card());
        let json = serde_json::to_value(&detail).unwrap();
        let obj = json.as_object().unwrap();
        for key in [
            "members",
            "tags",
            "checklists",
            "children",
            "parents",
            "blockers",
            "files",
        ] {
            assert!(!obj.contains_key(key), "{key} must be skipped when empty");
        }
    }

    #[test]
    fn mutation_result_builds_short_web_url() {
        let card = minimal_card();
        let result = MutationResult::new(&card, "https://example.kaiten.ru");
        assert_eq!(result.url, "https://example.kaiten.ru/1");
    }
}
