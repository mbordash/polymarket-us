use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolymarketUsError {
    #[error("authentication required for endpoint {0}")]
    MissingAuth(&'static str),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("authentication failed: {0}")]
    Authentication(String),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("rate limit exceeded: {0}")]
    RateLimited(String),
    #[error("internal server error: {0}")]
    Server(String),
    #[error("api error {status}: {message}")]
    Api { status: u16, message: String },
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
    #[error(transparent)]
    Decode(#[from] serde_json::Error),
}

impl PolymarketUsError {
    pub fn from_status(status: reqwest::StatusCode, message: String) -> Self {
        match status.as_u16() {
            400 => Self::BadRequest(message),
            401 => Self::Authentication(message),
            404 => Self::NotFound(message),
            429 => Self::RateLimited(message),
            500 | 502 | 503 | 504 => Self::Server(message),
            code => Self::Api {
                status: code,
                message,
            },
        }
    }
}
