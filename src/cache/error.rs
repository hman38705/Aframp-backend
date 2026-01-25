//! Cache-specific error types

use std::fmt;

/// Cache operation errors
#[derive(Debug)]
pub enum CacheError {
    /// Connection-related errors (Redis unavailable, network issues, etc.)
    ConnectionError(String),
    /// Serialization/deserialization errors
    SerializationError(String),
    /// Key-related errors (invalid key format, etc.)
    KeyError(String),
    /// TTL-related errors
    TtlError(String),
    /// Operation-specific errors
    OperationError(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::ConnectionError(msg) => write!(f, "Cache connection error: {}", msg),
            CacheError::SerializationError(msg) => write!(f, "Cache serialization error: {}", msg),
            CacheError::KeyError(msg) => write!(f, "Cache key error: {}", msg),
            CacheError::TtlError(msg) => write!(f, "Cache TTL error: {}", msg),
            CacheError::OperationError(msg) => write!(f, "Cache operation error: {}", msg),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<redis::RedisError> for CacheError {
    fn from(err: redis::RedisError) -> Self {
        CacheError::ConnectionError(err.to_string())
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(err: serde_json::Error) -> Self {
        CacheError::SerializationError(err.to_string())
    }
}

impl From<bb8::RunError<redis::RedisError>> for CacheError {
    fn from(err: bb8::RunError<redis::RedisError>) -> Self {
        CacheError::ConnectionError(format!("Pool error: {}", err))
    }
}

/// Result type alias for cache operations
pub type CacheResult<T> = Result<T, CacheError>;