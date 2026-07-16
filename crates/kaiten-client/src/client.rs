use std::time::{Duration, Instant};

use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::error::{KaitenError, Result};

const MAX_RETRIES: u32 = 3;
const MAX_RETRY_WAIT_SECS: u64 = 5;

/// How long to sleep before retrying a 429, given the `X-RateLimit-Reset`
/// header value (`None` when the header is missing or unparseable).
///
/// Honest clamp: sleeps for the actual reset window, capped at
/// `MAX_RETRY_WAIT_SECS` so a single retry never blocks for longer than that.
/// A missing/unparseable header falls back to a 1s wait.
fn retry_wait_secs(reset_secs: Option<u64>) -> u64 {
    match reset_secs {
        Some(secs) => secs.min(MAX_RETRY_WAIT_SECS),
        None => 1,
    }
}

/// HTTP client for the Kaiten API.
pub struct KaitenClient {
    http: reqwest::Client,
    base_url: url::Url,
    token: String,
}

impl std::fmt::Debug for KaitenClient {
    // Manual impl: never print the bearer token, even via `{:?}`, and skip the
    // `http` field since `reqwest::Client` carries nothing useful to debug.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KaitenClient")
            .field("base_url", &self.base_url)
            .field("token", &"***REDACTED***")
            .finish_non_exhaustive()
    }
}

impl KaitenClient {
    /// `base_url` WITHOUT a trailing slash, e.g. "https://mycompany.kaiten.ru/api/latest".
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let parsed = url::Url::parse(base_url)
            .map_err(|e| KaitenError::InvalidBaseUrl(format!("{base_url}: {e}")))?;
        Ok(Self {
            http: reqwest::Client::builder().build()?,
            base_url: parsed,
            token: token.to_string(),
        })
    }

    /// Base API URL this client talks to,
    /// e.g. "https://mycompany.kaiten.ru/api/latest".
    pub fn base_url(&self) -> &url::Url {
        &self.base_url
    }

    /// Raw request for `kaiten api`: `path` starts with "/", query is already in `path`.
    pub async fn raw(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.request(method, path, None, body).await
    }

    /// Spaces resource facade.
    pub fn spaces(&self) -> crate::api::spaces::Spaces<'_> {
        crate::api::spaces::Spaces { client: self }
    }

    /// Users resource facade.
    pub fn users(&self) -> crate::api::users::Users<'_> {
        crate::api::users::Users { client: self }
    }

    /// Boards resource facade.
    pub fn boards(&self) -> crate::api::boards::Boards<'_> {
        crate::api::boards::Boards { client: self }
    }

    /// Cards resource facade.
    pub fn cards(&self) -> crate::api::cards::Cards<'_> {
        crate::api::cards::Cards { client: self }
    }

    /// Comments resource facade.
    pub fn comments(&self) -> crate::api::comments::Comments<'_> {
        crate::api::comments::Comments { client: self }
    }

    /// Card members resource facade.
    pub fn members(&self) -> crate::api::members::Members<'_> {
        crate::api::members::Members { client: self }
    }

    /// Checklists resource facade.
    pub fn checklists(&self) -> crate::api::checklists::Checklists<'_> {
        crate::api::checklists::Checklists { client: self }
    }

    /// Card file attachments facade.
    pub fn files(&self) -> crate::api::files::Files<'_> {
        crate::api::files::Files { client: self }
    }

    /// Card links facade (children hierarchy, blockers).
    pub fn links(&self) -> crate::api::links::Links<'_> {
        crate::api::links::Links { client: self }
    }

    /// Custom properties facade.
    pub fn properties(&self) -> crate::api::properties::Properties<'_> {
        crate::api::properties::Properties { client: self }
    }

    /// Card time logs facade.
    pub fn time_logs(&self) -> crate::api::time_logs::TimeLogs<'_> {
        crate::api::time_logs::TimeLogs { client: self }
    }

    /// Tags and card types facade.
    pub fn tags(&self) -> crate::api::tags::Tags<'_> {
        crate::api::tags::Tags { client: self }
    }

    /// Retry-and-trace core shared by ALL requests (JSON and empty responses alike).
    /// Returns `(status, raw response body)` on 2xx; maps 4xx/5xx (except 429) to
    /// `Api { status, message, body }` and an exhausted 429 to `RateLimited`.
    /// `request_empty` (Task 8) also builds on this core.
    pub(crate) async fn send_with_retry(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<serde_json::Value>,
    ) -> Result<(u16, String)> {
        let url = format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path);
        let mut retries = 0u32;
        loop {
            let mut req = self
                .http
                .request(method.clone(), url.as_str())
                .bearer_auth(&self.token);
            if let Some(q) = &query {
                req = req.query(q);
            }
            if let Some(b) = &body {
                tracing::trace!(body = %b, "request body");
                req = req.json(b);
            }

            let started = Instant::now();
            let resp = req.send().await?;
            let status = resp.status();
            let elapsed = started.elapsed();
            tracing::debug!(
                method = %method,
                path,
                status = status.as_u16(),
                elapsed_ms = elapsed.as_secs() * 1000 + u64::from(elapsed.subsec_millis()),
                "http request"
            );

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                // The error carries the ACTUAL header value (missing/garbage -> 1);
                // only the sleep below is clamped to <=5s via `retry_wait_secs`.
                let reset_secs_raw = resp
                    .headers()
                    .get("X-RateLimit-Reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                let reset_secs = reset_secs_raw.unwrap_or(1);
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(KaitenError::RateLimited {
                        retry_after_secs: reset_secs,
                    });
                }
                let wait_secs = retry_wait_secs(reset_secs_raw);
                tracing::debug!(wait_secs, retry = retries, "rate limited, retrying");
                tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                continue;
            }

            let text = resp.text().await?;
            tracing::trace!(body = %text, "response body");

            if !status.is_success() {
                let message = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
                    .unwrap_or_else(|| {
                        if text.trim().is_empty() {
                            status
                                .canonical_reason()
                                .unwrap_or("unknown error")
                                .to_owned()
                        } else {
                            text.clone()
                        }
                    });
                return Err(KaitenError::Api {
                    status: status.as_u16(),
                    message,
                    body: text,
                });
            }

            return Ok((status.as_u16(), text));
        }
    }

    /// Multipart upload core (PUT with a single binary `field`).
    ///
    /// Separate from `send_with_retry` because `reqwest::multipart::Form`
    /// is not cloneable — the whole file is read into memory by the caller
    /// and the form is rebuilt from the bytes on every 429 retry. The retry
    /// policy, tracing and error mapping mirror `send_with_retry`.
    pub(crate) async fn send_multipart_put(
        &self,
        path: &str,
        field: &'static str,
        file_name: String,
        bytes: Vec<u8>,
    ) -> Result<String> {
        let url = format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path);
        let mut retries = 0u32;
        loop {
            let part = reqwest::multipart::Part::bytes(bytes.clone()).file_name(file_name.clone());
            let form = reqwest::multipart::Form::new().part(field, part);
            let started = Instant::now();
            let resp = self
                .http
                .put(url.as_str())
                .bearer_auth(&self.token)
                .multipart(form)
                .send()
                .await?;
            let status = resp.status();
            let elapsed = started.elapsed();
            tracing::debug!(
                method = "PUT(multipart)",
                path,
                status = status.as_u16(),
                elapsed_ms = elapsed.as_secs() * 1000 + u64::from(elapsed.subsec_millis()),
                "http request"
            );

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let reset_secs_raw = resp
                    .headers()
                    .get("X-RateLimit-Reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(KaitenError::RateLimited {
                        retry_after_secs: reset_secs_raw.unwrap_or(1),
                    });
                }
                let wait_secs = retry_wait_secs(reset_secs_raw);
                tracing::debug!(wait_secs, retry = retries, "rate limited, retrying");
                tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                continue;
            }

            let text = resp.text().await?;
            tracing::trace!(body = %text, "response body");
            if !status.is_success() {
                let message = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
                    .unwrap_or_else(|| {
                        if text.trim().is_empty() {
                            status
                                .canonical_reason()
                                .unwrap_or("unknown error")
                                .to_owned()
                        } else {
                            text.clone()
                        }
                    });
                return Err(KaitenError::Api {
                    status: status.as_u16(),
                    message,
                    body: text,
                });
            }
            return Ok(text);
        }
    }

    /// Decode a response body the same way `request<T>` does.
    pub(crate) fn decode<T: DeserializeOwned>(text: &str) -> Result<T> {
        let mut de = serde_json::Deserializer::from_str(text);
        serde_path_to_error::deserialize(&mut de).map_err(|e| KaitenError::Decode {
            path: e.path().to_string(),
            source: e.into_inner(),
        })
    }

    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let (_status, text) = self.send_with_retry(method, path, query, body).await?;
        Self::decode(&text)
    }

    /// Perform a request whose response body may be empty and is ignored
    /// (Kaiten DELETE endpoints return JSON or an empty body).
    ///
    /// Thin wrapper over `send_with_retry`, so the 429 retry loop, the
    /// request/response tracing and the non-2xx -> `Api` error mapping are
    /// all shared with `request<T>` (errors are mapped inside `send_with_retry`).
    pub(crate) async fn request_empty(&self, method: Method, path: &str) -> Result<()> {
        self.send_with_retry(method, path, None, None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::KaitenClient;
    use crate::error::KaitenError;

    #[derive(Debug, serde::Deserialize)]
    struct Probe {
        #[allow(dead_code)]
        id: u64,
    }

    #[tokio::test]
    async fn decode_error_reports_field_path() {
        let server = MockServer::start().await;
        // id приходит строкой вместо числа — Decode должен указать поле "id".
        Mock::given(method("GET"))
            .and(path("/probe"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"id":"not-a-number","title":"x","extra":true}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
        let err = client
            .request::<Probe>(reqwest::Method::GET, "/probe", None, None)
            .await
            .unwrap_err();

        match err {
            KaitenError::Decode { path, .. } => assert_eq!(path, "id"),
            other => panic!("expected Decode error, got {other:?}"),
        }
    }

    #[test]
    fn retry_wait_secs_clamps_to_five_seconds() {
        use super::retry_wait_secs;

        assert_eq!(retry_wait_secs(Some(10)), 5);
        assert_eq!(retry_wait_secs(Some(3)), 3);
        assert_eq!(retry_wait_secs(Some(0)), 0);
        assert_eq!(retry_wait_secs(None), 1);
    }
}
