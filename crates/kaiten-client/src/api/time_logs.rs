use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::TimeLog;

/// Card time logs facade. Construct via [`KaitenClient::time_logs`].
pub struct TimeLogs<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl TimeLogs<'_> {
    /// GET /cards/{card_id}/time-logs
    pub async fn list(&self, card_id: u64) -> Result<Vec<TimeLog>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/cards/{card_id}/time-logs"),
                None,
                None,
            )
            .await
    }

    /// POST /cards/{card_id}/time-logs
    ///
    /// `time_spent` is in MINUTES; `role_id` is required by the API
    /// (built-in roles are negative, e.g. -1 = Employee — see
    /// [`crate::api::users::Users::roles`]).
    pub async fn add(
        &self,
        card_id: u64,
        time_spent: i64,
        for_date: &str,
        role_id: i64,
        comment: Option<&str>,
    ) -> Result<TimeLog> {
        let mut body = serde_json::Map::new();
        body.insert("time_spent".into(), time_spent.into());
        body.insert("for_date".into(), for_date.into());
        body.insert("role_id".into(), role_id.into());
        if let Some(text) = comment {
            body.insert("comment".into(), text.into());
        }
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/time-logs"),
                None,
                Some(body.into()),
            )
            .await
    }

    /// DELETE /cards/{card_id}/time-logs/{time_log_id}
    pub async fn remove(&self, card_id: u64, time_log_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/time-logs/{time_log_id}"),
            )
            .await
    }
}
