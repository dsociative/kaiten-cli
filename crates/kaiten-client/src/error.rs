pub type Result<T> = std::result::Result<T, KaitenError>;

#[derive(Debug, thiserror::Error)]
pub enum KaitenError {
    /// message — из JSON-поля "message" (или reason-фраза при пустом теле);
    /// body — сырое тело ответа целиком (пустая строка, если тела нет).
    #[error("API error {status}: {message}")]
    Api {
        status: u16,
        message: String,
        body: String,
    },
    #[error("rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("failed to decode response at `{path}`: {source}")]
    Decode {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(String),
    /// Local filesystem failure while reading a file to upload.
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}
