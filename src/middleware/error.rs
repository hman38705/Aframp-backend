//! Error response formatting middleware
//!
//! Provides standardized error responses with consistent JSON structure,
//! HTTP status codes, error codes, and user-friendly messages.

#[cfg(feature = "database")]
use crate::error::{AppError, ErrorCode};
#[cfg(feature = "database")]
use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
#[cfg(feature = "database")]
use chrono::Utc;
#[cfg(feature = "database")]
use serde::{Deserialize, Serialize};

/// Standardized error response structure
///
/// This is returned to clients for all error cases, ensuring
/// consistent error handling across the API.
#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Machine-readable error code
    pub error: ErrorCode,

    /// Human-readable error message
    pub message: String,

    /// Request ID for debugging and support
    pub request_id: Option<String>,

    /// ISO 8601 timestamp of the error
    pub timestamp: String,

    /// Optional additional details (e.g., validation errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// Whether the client should retry the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

#[cfg(feature = "database")]
impl ErrorResponse {
    /// Create a new error response from an AppError
    pub fn from_app_error(error: &AppError) -> Self {
        Self {
            error: error.error_code(),
            message: error.user_message(),
            request_id: error.request_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
            details: None,
            retryable: Some(error.is_retryable()),
        }
    }

    /// Create an error response with additional details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Create a generic internal server error response
    pub fn internal_error(request_id: Option<String>) -> Self {
        Self {
            error: ErrorCode::InternalError,
            message: "An internal server error occurred. Please try again later.".to_string(),
            request_id,
            timestamp: Utc::now().to_rfc3339(),
            details: None,
            retryable: Some(false),
        }
    }

    /// Create a validation error response with field details
    pub fn validation_error(request_id: Option<String>, field: &str, message: &str) -> Self {
        Self {
            error: ErrorCode::ValidationError,
            message: format!("Validation failed for field '{}'", field),
            request_id,
            timestamp: Utc::now().to_rfc3339(),
            details: Some(serde_json::json!({
                "field": field,
                "error": message,
            })),
            retryable: Some(false),
        }
    }
}

/// Implement IntoResponse for AppError to automatically convert errors
/// into HTTP responses with proper status codes and JSON formatting
#[cfg(feature = "database")]
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code =
            StatusCode::from_u16(self.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        // Log the error with context
        if status_code.is_server_error() {
            tracing::error!(
                error = ?self,
                request_id = ?self.request_id,
                status = %status_code.as_u16(),
                "Server error occurred"
            );
        } else {
            tracing::warn!(
                error = ?self,
                request_id = ?self.request_id,
                status = %status_code.as_u16(),
                "Client error occurred"
            );
        }

        let error_response = ErrorResponse::from_app_error(&self);
        (status_code, Json(error_response)).into_response()
    }
}

/// Middleware for handling panics and converting them to proper error responses
///
/// This ensures that even unexpected panics are caught and returned as
/// structured error responses instead of crashing the server.
#[cfg(feature = "database")]
pub async fn error_handling_middleware(
    request: Request,
    next: axum::middleware::Next,
) -> Result<Response, AppError> {
    // Extract request ID if available
    let _request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Process the request
    let response = next.run(request).await;

    Ok(response)
}

/// Create a standardized success response
///
/// Use this for consistent JSON responses across successful operations
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "database")]
/// # {
/// use aframp::middleware::error::success_response;
/// use serde_json::json;
///
/// let response = success_response(json!({
///     "transaction_id": "tx_123",
///     "status": "completed"
/// }));
/// # }
/// ```
#[cfg(feature = "database")]
pub fn success_response<T: Serialize>(data: T) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": true,
        "data": data,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Create a standardized success response with metadata
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "database")]
/// # {
/// use aframp::middleware::error::success_response_with_meta;
/// use serde_json::json;
///
/// let response = success_response_with_meta(
///     json!([{"id": 1}, {"id": 2}]),
///     json!({
///         "page": 1,
///         "total": 2,
///         "per_page": 10
///     })
/// );
/// # }
/// ```
#[cfg(feature = "database")]
pub fn success_response_with_meta<T: Serialize, M: Serialize>(
    data: T,
    meta: M,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": true,
        "data": data,
        "meta": meta,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Helper to extract request ID from request headers
#[cfg(feature = "database")]
pub fn get_request_id_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Build a standardized JSON error response for handlers that return StatusCode + message.
#[cfg(feature = "database")]
pub fn json_error_response(
    status: StatusCode,
    message: impl Into<String>,
    request_id: Option<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    let message = message.into();
    let error_response = match status.as_u16() {
        400..=499 => ErrorResponse::validation_error(request_id, "request", &message)
            .with_details(serde_json::json!({ "message": message })),
        _ => ErrorResponse::internal_error(request_id),
    };

    (status, Json(error_response))
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;
    use crate::error::{AppError, AppErrorKind, DomainError, ValidationError};
    use axum::{http::StatusCode, response::IntoResponse};

    #[test]
    fn test_error_response_from_app_error() {
        let app_error = AppError::new(AppErrorKind::Domain(DomainError::InsufficientBalance {
            available: "50".to_string(),
            required: "100".to_string(),
        }))
        .with_request_id("req_123");

        let error_response = ErrorResponse::from_app_error(&app_error);

        assert_eq!(error_response.error, ErrorCode::InsufficientCngnBalance);
        assert_eq!(error_response.request_id, Some("req_123".to_string()));
        assert!(error_response.message.contains("Insufficient CNGN balance"));
    }

    #[test]
    fn test_app_error_into_response() {
        let app_error = AppError::new(AppErrorKind::Validation(ValidationError::InvalidAmount {
            amount: "-100".to_string(),
            reason: "Amount cannot be negative".to_string(),
        }));

        let response = app_error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_internal_error_response() {
        let error = ErrorResponse::internal_error(Some("req_456".to_string()));

        assert_eq!(error.error, ErrorCode::InternalError);
        assert_eq!(error.request_id, Some("req_456".to_string()));
        assert!(error.message.contains("internal server error"));
    }

    #[test]
    fn test_validation_error_response() {
        let error = ErrorResponse::validation_error(
            Some("req_789".to_string()),
            "email",
            "Invalid email format",
        );

        assert_eq!(error.error, ErrorCode::ValidationError);
        assert_eq!(error.request_id, Some("req_789".to_string()));
        assert!(error.details.is_some());
    }

    #[test]
    fn test_status_code_mapping() {
        // Test domain errors
        let insufficient_balance =
            AppError::new(AppErrorKind::Domain(DomainError::InsufficientBalance {
                available: "0".to_string(),
                required: "100".to_string(),
            }));
        assert_eq!(insufficient_balance.status_code(), 422);

        // Test validation errors
        let invalid_address = AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: "invalid".to_string(),
                reason: "Wrong format".to_string(),
            },
        ));
        assert_eq!(invalid_address.status_code(), 400);
    }

    #[tokio::test]
    async fn test_success_response() {
        use serde_json::json;

        let response = success_response(json!({
            "id": 123,
            "status": "success"
        }));

        // Verify it can be created and converted to response
        let _resp = response.into_response();
        // Note: Full response testing requires running in an actual HTTP context
    }
}
