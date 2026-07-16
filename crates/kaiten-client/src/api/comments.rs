use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Comment;

/// Comments resource facade. Construct via [`KaitenClient::comments`].
pub struct Comments<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Comments<'_> {
    /// GET /cards/{id}/comments
    pub async fn list(&self, card_id: u64) -> Result<Vec<Comment>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/cards/{card_id}/comments"),
                None,
                None,
            )
            .await
    }

    /// POST /cards/{id}/comments
    pub async fn add(&self, card_id: u64, text: &str) -> Result<Comment> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/comments"),
                None,
                Some(serde_json::json!({ "text": text })),
            )
            .await
    }

    /// PATCH /cards/{id}/comments/{comment_id}
    pub async fn update(&self, card_id: u64, comment_id: u64, text: &str) -> Result<Comment> {
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}/comments/{comment_id}"),
                None,
                Some(serde_json::json!({ "text": text })),
            )
            .await
    }

    /// DELETE /cards/{id}/comments/{comment_id}
    pub async fn remove(&self, card_id: u64, comment_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/comments/{comment_id}"),
            )
            .await
    }
}
