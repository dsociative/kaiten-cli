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
