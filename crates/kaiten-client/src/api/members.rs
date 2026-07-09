use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::CardMember;

/// Card members resource facade. Construct via [`KaitenClient::members`].
pub struct Members<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Members<'_> {
    /// POST /cards/{id}/members
    pub async fn add(&self, card_id: u64, user_id: u64) -> Result<CardMember> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/members"),
                None,
                Some(serde_json::json!({ "user_id": user_id })),
            )
            .await
    }

    /// DELETE /cards/{id}/members/{user_id}; the response body is ignored.
    pub async fn remove(&self, card_id: u64, user_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/members/{user_id}"),
            )
            .await
    }
}
