//! Error type for the [`super::client::StellarClient`] compatibility shim.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StellarError {
    #[error("transaction not found or failed: {reason}")]
    TransactionFailed { reason: String },

    #[error("account not found: {address}")]
    AccountNotFound { address: String },

    #[error("invalid Stellar address: {address}")]
    InvalidAddress { address: String },

    #[error("Horizon network error: {message}")]
    NetworkError { message: String },

    #[error("Horizon request timed out: {message}")]
    TimeoutError { message: String },

    #[error("Horizon rate limit exceeded")]
    RateLimitError,

    #[error("transaction signing error: {0}")]
    SigningError(String),

    #[error("Stellar client error: {0}")]
    Other(String),
}

impl StellarError {
    pub fn transaction_failed(reason: impl Into<String>) -> Self {
        StellarError::TransactionFailed {
            reason: reason.into(),
        }
    }

    pub fn signing_error(message: impl Into<String>) -> Self {
        StellarError::SigningError(message.into())
    }

    pub fn network_error(message: impl Into<String>) -> Self {
        StellarError::NetworkError {
            message: message.into(),
        }
    }

    pub fn invalid_address(address: impl Into<String>) -> Self {
        StellarError::InvalidAddress {
            address: address.into(),
        }
    }
}

impl From<crate::stellar::error::SubmissionError> for StellarError {
    fn from(err: crate::stellar::error::SubmissionError) -> Self {
        StellarError::Other(err.to_string())
    }
}
