use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::ExternalLink;

/// Card external links facade (`Links (common links)` in Kaiten). Construct
/// via [`KaitenClient::external_links`].
///
/// The API itself neither validates `url` nor rejects duplicates — this
/// client does not second-guess it.
pub struct ExternalLinks<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl ExternalLinks<'_> {
    /// GET /cards/{card_id}/external-links
    pub async fn list(&self, card_id: u64) -> Result<Vec<ExternalLink>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/cards/{card_id}/external-links"),
                None,
                None,
            )
            .await
    }

    /// POST /cards/{card_id}/external-links — `description` is sent only when given.
    pub async fn add(
        &self,
        card_id: u64,
        url: &str,
        description: Option<&str>,
    ) -> Result<ExternalLink> {
        let mut body = serde_json::json!({ "url": url });
        if let Some(description) = description {
            body["description"] = serde_json::Value::String(description.to_owned());
        }
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/external-links"),
                None,
                Some(body),
            )
            .await
    }

    /// PATCH /cards/{card_id}/external-links/{link_id} — only the given fields
    /// are sent (the API answers 404 to PUT).
    pub async fn update(
        &self,
        card_id: u64,
        link_id: u64,
        url: Option<&str>,
        description: Option<&str>,
    ) -> Result<ExternalLink> {
        let mut body = serde_json::Map::new();
        if let Some(url) = url {
            body.insert("url".into(), serde_json::Value::String(url.to_owned()));
        }
        if let Some(description) = description {
            body.insert(
                "description".into(),
                serde_json::Value::String(description.to_owned()),
            );
        }
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}/external-links/{link_id}"),
                None,
                Some(serde_json::Value::Object(body)),
            )
            .await
    }

    /// DELETE /cards/{card_id}/external-links/{link_id}
    pub async fn remove(&self, card_id: u64, link_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/external-links/{link_id}"),
            )
            .await
    }
}
