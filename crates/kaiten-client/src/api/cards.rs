use crate::client::KaitenClient;
use crate::error::{KaitenError, Result};
use crate::models::Card;

/// Filter for GET /cards. `None`/empty fields are omitted from the query.
#[derive(Debug, Default, Clone)]
pub struct CardFilter {
    pub space_id: Option<u64>,
    pub board_id: Option<u64>,
    pub column_id: Option<u64>,
    pub lane_id: Option<u64>,
    pub query: Option<String>,
    /// Serialized as a comma-separated list: "1,2,3".
    pub member_ids: Vec<u64>,
    pub owner_id: Option<u64>,
    /// Tag name.
    pub tag: Option<String>,
    pub type_id: Option<u64>,
    pub archived: Option<bool>,
    pub condition: Option<u8>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl CardFilter {
    pub fn to_query(&self) -> Vec<(String, String)> {
        fn push<T: ToString>(q: &mut Vec<(String, String)>, key: &str, value: Option<&T>) {
            if let Some(v) = value {
                q.push((key.to_string(), v.to_string()));
            }
        }

        let mut q: Vec<(String, String)> = Vec::new();
        push(&mut q, "space_id", self.space_id.as_ref());
        push(&mut q, "board_id", self.board_id.as_ref());
        push(&mut q, "column_id", self.column_id.as_ref());
        push(&mut q, "lane_id", self.lane_id.as_ref());
        push(&mut q, "query", self.query.as_ref());
        if !self.member_ids.is_empty() {
            q.push((
                "member_ids".to_string(),
                self.member_ids
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }
        push(&mut q, "owner_id", self.owner_id.as_ref());
        push(&mut q, "tag", self.tag.as_ref());
        push(&mut q, "type_id", self.type_id.as_ref());
        push(&mut q, "archived", self.archived.as_ref());
        push(&mut q, "condition", self.condition.as_ref());
        push(&mut q, "limit", self.limit.as_ref());
        push(&mut q, "offset", self.offset.as_ref());
        q
    }
}

/// Body for POST /cards. All fields except `board_id`/`title` are optional
/// and omitted from JSON when `None`.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CreateCard {
    pub board_id: u64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asap: Option<bool>,
}

/// Body for PATCH /cards/{id}. Move = update with `column_id`/`lane_id`/`board_id`;
/// archive = update with `condition = 2`. `None` fields are omitted from JSON.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct UpdateCard {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_id: Option<u64>,
    /// 2 = archive the card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<u8>,
}

/// Cards resource facade. Construct via [`KaitenClient::cards`].
pub struct Cards<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Cards<'_> {
    /// GET /cards
    pub async fn list(&self, filter: &CardFilter) -> Result<Vec<Card>> {
        let q = filter.to_query();
        let query = if q.is_empty() { None } else { Some(q) };
        self.client
            .request(reqwest::Method::GET, "/cards", query, None)
            .await
    }

    /// GET /cards/{id}
    pub async fn get(&self, card_id: u64) -> Result<Card> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/cards/{card_id}"),
                None,
                None,
            )
            .await
    }

    /// POST /cards
    pub async fn create(&self, req: &CreateCard) -> Result<Card> {
        let body = serde_json::to_value(req).map_err(|e| KaitenError::Decode {
            path: "CreateCard".to_string(),
            source: e,
        })?;
        self.client
            .request(reqwest::Method::POST, "/cards", None, Some(body))
            .await
    }

    /// PATCH /cards/{id}
    pub async fn update(&self, card_id: u64, req: &UpdateCard) -> Result<Card> {
        let body = serde_json::to_value(req).map_err(|e| KaitenError::Decode {
            path: "UpdateCard".to_string(),
            source: e,
        })?;
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}"),
                None,
                Some(body),
            )
            .await
    }
}
