// SPDX-License-Identifier: Apache-2.0
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubgraphError {
    #[error("API key not set")]
    ApiKeyNotSet,
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("GraphQL error: {0}")]
    GraphQlError(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Internal processing error: {0}")]
    InternalProcessingError(String),
}
