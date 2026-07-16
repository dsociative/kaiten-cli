use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::{CustomProperty, SelectValue};

/// Custom properties resource facade. Construct via [`KaitenClient::properties`].
///
/// Custom properties live at the COMPANY level (not per space).
pub struct Properties<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Properties<'_> {
    /// GET /company/custom-properties
    pub async fn list(&self) -> Result<Vec<CustomProperty>> {
        self.client
            .request(
                reqwest::Method::GET,
                "/company/custom-properties",
                None,
                None,
            )
            .await
    }

    /// GET /company/custom-properties/{property_id}/select-values
    pub async fn select_values(&self, property_id: u64) -> Result<Vec<SelectValue>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/company/custom-properties/{property_id}/select-values"),
                None,
                None,
            )
            .await
    }
}
