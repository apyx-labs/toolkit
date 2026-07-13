use thiserror::Error;

use crate::model::{ExecutionId, ExecutionState};

#[derive(Debug, Error)]
pub enum Error {
    #[cfg(feature = "reqwest")]
    #[error("API key is not a valid header value: {0}")]
    InvalidApiKey(#[source] reqwest::header::InvalidHeaderValue),
    #[cfg(feature = "reqwest")]
    #[error("error sending request: {0}")]
    ExecuteRequest(#[source] reqwest_middleware::Error),
    #[cfg(feature = "reqwest")]
    #[error("failed to deserialize response: {0}")]
    DeserializeResponse(#[source] reqwest::Error),
    #[error("failed to serialize rows: {0}")]
    SerializeRows(#[source] serde_json::Error),
    #[cfg(feature = "reqwest")]
    #[error("API error (HTTP {status}): {body}")]
    Api {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("execution {execution_id} ended with state: {state}")]
    ExecutionEnded {
        execution_id: ExecutionId,
        state: ExecutionState,
    },
}
