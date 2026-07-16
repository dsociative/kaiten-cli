use std::path::Path;

use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::CardFile;

/// Card file attachments facade. Construct via [`KaitenClient::files`].
///
/// SECURITY: Kaiten serves uploaded files from a public (unguessable) URL
/// without authentication — never attach secrets.
pub struct Files<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Files<'_> {
    /// PUT /cards/{card_id}/files — multipart upload, binary field `file`.
    ///
    /// Reads the whole file into memory (uploads are interactive-sized;
    /// the 429 retry loop needs the bytes to rebuild the form).
    pub async fn attach(&self, card_id: u64, file_path: &Path) -> Result<CardFile> {
        let bytes = tokio::fs::read(file_path).await?;
        let file_name = file_path
            .file_name()
            .map_or_else(|| "file".to_string(), |n| n.to_string_lossy().into_owned());
        let text = self
            .client
            .send_multipart_put(&format!("/cards/{card_id}/files"), "file", file_name, bytes)
            .await?;
        KaitenClient::decode(&text)
    }

    /// DELETE /cards/{card_id}/files/{file_id}
    pub async fn detach(&self, card_id: u64, file_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/files/{file_id}"),
            )
            .await
    }
}
