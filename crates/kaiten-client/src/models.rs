//! All Kaiten API models.
//!
//! Deserialization is tolerant: unknown fields are ignored (no
//! `deny_unknown_fields`), fields that may be absent in a particular
//! response are `Option<...>` with `#[serde(default)]`.
//! Dates are plain ISO strings (no chrono).

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: u64,
    pub uid: String,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub activated: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Space {
    pub id: u64,
    pub uid: String,
    pub title: String,
    #[serde(default)]
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(rename = "type", default)]
    pub column_type: Option<u8>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Lane {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// A nested `board` inside a card has no `columns`/`lanes` keys,
/// so both default to empty vectors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Board {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub default_card_type_id: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardType {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub letter: Option<String>,
    #[serde(default)]
    pub color: Option<i64>,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// A tag inside `card.tags`: `id` is the link id, `tag_id` is the company tag id.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardTag {
    pub id: u64,
    #[serde(default)]
    pub tag_id: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub color: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardMember {
    /// User id.
    pub id: u64,
    #[serde(default)]
    pub user_id: Option<u64>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    /// 2 = responsible
    #[serde(rename = "type", default)]
    pub member_type: Option<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChecklistItem {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub checked: Option<bool>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checklist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub items: Vec<ChecklistItem>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// GET /cards/{id} returns the full card; GET /cards returns cards
/// without `description`/`members`/`checklists` — the same model parses both.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Card {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub asap: Option<bool>,
    #[serde(default)]
    pub archived: Option<bool>,
    /// 1 = live, 2 = archived
    #[serde(default)]
    pub condition: Option<u8>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(default)]
    pub state: Option<u8>,
    #[serde(default)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub column_id: Option<u64>,
    #[serde(default)]
    pub lane_id: Option<u64>,
    #[serde(default)]
    pub type_id: Option<u64>,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub comments_total: Option<u32>,
    /// Nested board has no `columns`/`lanes` keys → they default to empty.
    #[serde(default)]
    pub board: Option<Board>,
    #[serde(default)]
    pub column: Option<Column>,
    #[serde(default)]
    pub lane: Option<Lane>,
    #[serde(rename = "type", default)]
    pub card_type: Option<CardType>,
    #[serde(default)]
    pub owner: Option<User>,
    #[serde(default)]
    pub members: Vec<CardMember>,
    #[serde(default)]
    pub tags: Vec<CardTag>,
    #[serde(default)]
    pub checklists: Vec<Checklist>,
    /// Custom properties, read-only.
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub edited: Option<bool>,
    #[serde(default)]
    pub author: Option<User>,
    #[serde(default)]
    pub author_id: Option<u64>,
}
