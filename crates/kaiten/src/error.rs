#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] kaiten_client::KaitenError),
    #[error("config: {0}")]
    Config(String),
    #[error("{0}")]
    InvalidArg(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
