use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::{Checklist, ChecklistItem};

/// Checklists resource facade. Construct via [`KaitenClient::checklists`].
///
/// NOTE: `GET /cards/{id}/checklists` does NOT exist (the API answers 405).
/// Read checklists from `Card.checklists` via `cards().get()`.
pub struct Checklists<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Checklists<'_> {
    /// POST /cards/{id}/checklists
    pub async fn add(&self, card_id: u64, name: &str) -> Result<Checklist> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/checklists"),
                None,
                Some(serde_json::json!({ "name": name })),
            )
            .await
    }

    /// POST /cards/{card_id}/checklists/{checklist_id}/items
    pub async fn add_item(
        &self,
        card_id: u64,
        checklist_id: u64,
        text: &str,
    ) -> Result<ChecklistItem> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/checklists/{checklist_id}/items"),
                None,
                Some(serde_json::json!({ "text": text })),
            )
            .await
    }

    /// PATCH /cards/{card_id}/checklists/{checklist_id}/items/{item_id}
    pub async fn set_item_checked(
        &self,
        card_id: u64,
        checklist_id: u64,
        item_id: u64,
        checked: bool,
    ) -> Result<ChecklistItem> {
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}/checklists/{checklist_id}/items/{item_id}"),
                None,
                Some(serde_json::json!({ "checked": checked })),
            )
            .await
    }
}
