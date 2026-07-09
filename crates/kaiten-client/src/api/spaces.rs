use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Space;

/// Spaces resource facade. Construct via [`KaitenClient::spaces`].
pub struct Spaces<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Spaces<'_> {
    /// GET /spaces
    pub async fn list(&self) -> Result<Vec<Space>> {
        self.client
            .request(reqwest::Method::GET, "/spaces", None, None)
            .await
    }
}
