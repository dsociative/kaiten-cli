use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::{CardType, Tag};

/// Tags and card types facade. Construct via [`KaitenClient::tags`].
pub struct Tags<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Tags<'_> {
    /// GET /tags — company tags.
    pub async fn list(&self) -> Result<Vec<Tag>> {
        self.client
            .request(reqwest::Method::GET, "/tags", None, None)
            .await
    }

    /// POST /cards/{id}/tags — adds by name; creates the company tag if missing.
    pub async fn add_to_card(&self, card_id: u64, name: &str) -> Result<Tag> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/tags"),
                None,
                Some(serde_json::json!({ "name": name })),
            )
            .await
    }

    /// DELETE /cards/{id}/tags/{tag_id}; the response body is ignored.
    pub async fn remove_from_card(&self, card_id: u64, tag_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/tags/{tag_id}"),
            )
            .await
    }

    /// GET /card-types
    pub async fn card_types(&self) -> Result<Vec<CardType>> {
        self.client
            .request(reqwest::Method::GET, "/card-types", None, None)
            .await
    }
}
