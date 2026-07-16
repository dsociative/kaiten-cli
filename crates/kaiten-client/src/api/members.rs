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

    /// PATCH /cards/{id}/members/{user_id} — change the member role
    /// (type 2 = responsible, 1 = regular member). Live-verified format.
    pub async fn update_role(
        &self,
        card_id: u64,
        user_id: u64,
        responsible: bool,
    ) -> Result<CardMember> {
        let member_type = if responsible { 2 } else { 1 };
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}/members/{user_id}"),
                None,
                Some(serde_json::json!({ "type": member_type })),
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
