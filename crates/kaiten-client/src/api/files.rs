use std::path::Path;

use crate::client::KaitenClient;
use crate::error::{KaitenError, Result};
use crate::models::{CardFile, FileRef};

/// Card file attachments facade. Construct via [`KaitenClient::files`].
///
/// SECURITY: Kaiten's classic storage serves uploaded files from a public
/// (unguessable) URL without authentication — never attach secrets. The
/// newer storage serves files through an authenticated API path that
/// redirects to storage; [`Files::download`] sends the API token only to
/// the API origin, never to a file or storage host.
pub struct Files<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Files<'_> {
    /// Attachments of a card, read from `GET /cards/{card_id}` — the
    /// dedicated `GET /cards/{card_id}/files` returns `[]` on the newer
    /// storage (issue #12), so it is not used.
    pub async fn list(&self, card_id: u64) -> Result<Vec<CardFile>> {
        Ok(self.client.cards().get(card_id).await?.files)
    }

    /// Download an attachment's content. An absolute `url` (classic storage)
    /// is fetched as is; a host-root-relative one (newer storage) is resolved
    /// against the API origin. The token goes only to the API origin — never
    /// to the public file host, nor to a storage host reached through a
    /// redirect. The whole body is held in memory, as with [`Files::attach`].
    pub async fn download(&self, file: &CardFile) -> Result<Vec<u8>> {
        let raw = file.url.as_deref().ok_or_else(|| {
            invalid_input(format!(
                "attachment `{}` ({}) has no download url",
                file.name,
                FileRef::from(file)
            ))
        })?;
        let url = resolve_file_url(self.client.base_url(), raw)?;
        self.client.get_bytes(&url).await
    }

    /// [`Files::download`] straight into `path` (created or truncated;
    /// parent directories are not created).
    pub async fn download_to(&self, file: &CardFile, path: &Path) -> Result<()> {
        let bytes = self.download(file).await?;
        tokio::fs::write(path, bytes).await.map_err(|e| {
            KaitenError::Io(std::io::Error::new(
                e.kind(),
                format!("{}: {e}", path.display()),
            ))
        })
    }

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

/// Absolute URLs pass through; a path is resolved against the API **origin**
/// (`/api/v1/...` lives next to, not under, the `/api/latest` base) and must
/// stay there — WHATWG parsing would otherwise let `/\host/x` wander off to
/// another host, so the invariant is enforced here and not left to the
/// caller's token check.
pub(crate) fn resolve_file_url(base_url: &url::Url, raw: &str) -> Result<url::Url> {
    match url::Url::parse(raw) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let resolved = base_url
                .join(&format!("/{}", raw.trim_start_matches('/')))
                .map_err(|e| invalid_input(format!("attachment url `{raw}` is not valid: {e}")))?;
            if resolved.origin() == base_url.origin() {
                Ok(resolved)
            } else {
                Err(invalid_input(format!(
                    "attachment url `{raw}` does not resolve to the API origin"
                )))
            }
        }
        Err(e) => Err(invalid_input(format!(
            "attachment url `{raw}` is not valid: {e}"
        ))),
    }
}

fn invalid_input(message: String) -> KaitenError {
    KaitenError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    ))
}

#[cfg(test)]
mod tests {
    use super::resolve_file_url;

    fn base() -> url::Url {
        url::Url::parse("https://acme.kaiten.ru/api/latest").unwrap()
    }

    #[test]
    fn absolute_url_is_kept_verbatim() {
        let url = resolve_file_url(&base(), "https://files.kaiten.ru/abc.txt").unwrap();
        assert_eq!(url.as_str(), "https://files.kaiten.ru/abc.txt");
    }

    #[test]
    fn root_relative_url_uses_api_origin_not_base_path() {
        let url = resolve_file_url(&base(), "/api/v1/cards/u/files/f").unwrap();
        assert_eq!(
            url.as_str(),
            "https://acme.kaiten.ru/api/v1/cards/u/files/f"
        );
    }

    #[test]
    fn relative_url_without_leading_slash_is_rooted() {
        let url = resolve_file_url(&base(), "api/v1/cards/u/files/f").unwrap();
        assert_eq!(
            url.as_str(),
            "https://acme.kaiten.ru/api/v1/cards/u/files/f"
        );
    }

    /// WHATWG parsing turns `/\\host/x` into `https://host/x`: anything that
    /// does not land on the API origin must be refused here, not one call away.
    #[test]
    fn backslash_tricks_cannot_escape_the_api_origin() {
        for raw in [
            "/\\evil.host/x",
            "\\\\evil.host\\x",
            "\\/evil.host/x",
            "//evil.host/x",
        ] {
            match resolve_file_url(&base(), raw) {
                Ok(url) => assert_eq!(
                    url.origin(),
                    base().origin(),
                    "{raw} resolved off-origin to {url}"
                ),
                Err(crate::error::KaitenError::Io(e)) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput, "{raw}: {e}");
                }
                Err(other) => panic!("{raw}: unexpected {other:?}"),
            }
        }
    }

    #[test]
    fn garbage_url_is_invalid_input_io_error() {
        let err = resolve_file_url(&base(), "http://[").unwrap_err();
        assert!(
            matches!(&err, crate::error::KaitenError::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput),
            "{err:?}"
        );
    }
}
