use reqwest::StatusCode;
use thiserror::Error;

/// Errors returned by the Safe Transaction Service client.
#[derive(Debug, Error)]
pub enum Error {
    #[error("API key is not a valid header value: {0}")]
    InvalidApiKey(#[source] reqwest::header::InvalidHeaderValue),
    #[error("invalid base URL: {0}")]
    InvalidBaseUrl(#[source] url::ParseError),
    #[error("error sending request: {0}")]
    SendRequest(#[source] reqwest_middleware::Error),
    #[error("failed to deserialize response: {0}")]
    DeserializeResponse(#[source] reqwest::Error),
    #[error("failed to serialize request: {0}")]
    SerializeRequest(#[source] serde_json::Error),
    #[error("API error (HTTP {status}): {body}")]
    Api { status: StatusCode, body: String },
    #[error("failed to sign SafeTx digest: {0}")]
    Signing(#[source] alloy::signers::Error),
}
