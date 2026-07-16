use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::{Blocker, Card};

/// Card links facade: children hierarchy and blockers.
/// Construct via [`KaitenClient::links`].
///
/// There is no separate "parents" API — making X a parent of Y is
/// `add_child(X, Y)`; parents are read from the full card.
pub struct Links<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Links<'_> {
    /// POST /cards/{parent_id}/children — make `child_id` a child of `parent_id`.
    pub async fn add_child(&self, parent_id: u64, child_id: u64) -> Result<Card> {
        let body = serde_json::json!({ "card_id": child_id });
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{parent_id}/children"),
                None,
                Some(body),
            )
            .await
    }

    /// DELETE /cards/{parent_id}/children/{child_id}
    pub async fn remove_child(&self, parent_id: u64, child_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{parent_id}/children/{child_id}"),
            )
            .await
    }

    /// POST /cards/{card_id}/blockers — block `card_id` by another card
    /// and/or a free-text reason (the API requires at least one of the two).
    pub async fn add_blocker(
        &self,
        card_id: u64,
        blocker_card_id: Option<u64>,
        reason: Option<&str>,
    ) -> Result<Blocker> {
        let mut body = serde_json::Map::new();
        if let Some(id) = blocker_card_id {
            body.insert("blocker_card_id".into(), id.into());
        }
        if let Some(text) = reason {
            body.insert("reason".into(), text.into());
        }
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/blockers"),
                None,
                Some(body.into()),
            )
            .await
    }

    /// DELETE /cards/{card_id}/blockers/{blocker_id} — remove one blocker.
    pub async fn remove_blocker(&self, card_id: u64, blocker_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/blockers/{blocker_id}"),
            )
            .await
    }
}
