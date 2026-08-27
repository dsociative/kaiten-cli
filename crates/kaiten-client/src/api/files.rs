use std::path::Path;

use crate::client::KaitenClient;
use crate::error::{KaitenError, Result};
use crate::models::{CardFile, FileRef};

/// Card file attachments facade. Construct via [`KaitenClient::files`].
///
/// SECURITY: Kaiten's classic storage serves uploaded files from a public
/// (unguessable) URL without authentication — never attach secrets. The
/// newer storage answers its authenticated API path with the file's
/// metadata, whose `url` is a signed storage link valid for seconds;
/// [`Files::download`] sends the API token only to the API origin and
/// fetches the signed link without credentials.
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
    /// is fetched as is. A host-root-relative one (newer storage) is resolved
    /// against the API origin, where the API answers with the file's
    /// metadata rather than its bytes: the `url` in it is a signed storage
    /// link that expires within seconds, fetched immediately and without
    /// credentials. The token goes only to the API origin — never to the
    /// public file host nor to storage. The whole body is held in memory, as
    /// with [`Files::attach`].
    pub async fn download(&self, file: &CardFile) -> Result<Vec<u8>> {
        let raw = file.url.as_deref().ok_or_else(|| {
            invalid_input(format!(
                "attachment `{}` ({}) has no download url",
                file.name,
                FileRef::from(file)
            ))
        })?;
        let url = resolve_file_url(self.client.base_url(), raw)?;
        let fetched = self.client.get_bytes(&url).await?;
        // Metadata comes only from the API itself: the answer must be JSON and
        // must have been served by the very url we asked for. A redirect means
        // the API handed us over to storage, and whatever comes back — even a
        // JSON attachment — is the file.
        let is_metadata = fetched.url == url
            && url.origin() == self.client.base_url().origin()
            && is_json(fetched.content_type.as_deref());
        if !is_metadata {
            return Ok(fetched.bytes);
        }
        let text = String::from_utf8(fetched.bytes).map_err(|e| {
            invalid_input(format!(
                "file metadata for `{}` is not UTF-8: {e}",
                file.name
            ))
        })?;
        let location: FileLocation = KaitenClient::decode(&text)?;
        let signed = location.url.ok_or_else(|| {
            invalid_input(format!(
                "file metadata for `{}` ({}) has no download url",
                file.name,
                FileRef::from(file)
            ))
        })?;
        let signed = url::Url::parse(&signed)
            .map_err(|e| invalid_input(format!("storage url `{signed}` is not valid: {e}")))?;
        // The signed link is valid for seconds; it is fetched right away, and
        // the API-side 429 retries all happen before it is minted.
        match self.client.get_bytes(&signed).await {
            Ok(fetched) => Ok(fetched.bytes),
            Err(KaitenError::Api {
                status,
                message,
                body,
            }) => Err(KaitenError::Api {
                status,
                message: format!(
                    "storage refused the signed link for `{}` (it is valid only for seconds): {message}",
                    file.name
                ),
                body,
            }),
            Err(other) => Err(other),
        }
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

/// What the newer storage's API path returns for a file: its metadata, of
/// which only the signed storage `url` matters here.
#[derive(serde::Deserialize)]
struct FileLocation {
    url: Option<String>,
}

/// Media types are case-insensitive; parameters (`; charset=…`) are ignored.
fn is_json(content_type: Option<&str>) -> bool {
    content_type
        .and_then(|ct| ct.split(';').next())
        .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
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
