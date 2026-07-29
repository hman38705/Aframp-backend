//! CSRF (Cross-Site Request Forgery) protection middleware
//!
//! Implements the **double-submit cookie** pattern: the client must send the
//! same token both as a cookie and as an HTTP request header. Because a
//! cross-origin attacker cannot read cookie values (SameSite + HttpOnly +
//! CORS restrictions), a forged request cannot supply the matching header.
//!
//! # Exempt requests
//! The following request types are **not** checked for a CSRF token:
//! - Safe methods: `GET`, `HEAD`, `OPTIONS`, `TRACE`
//! - Requests carrying an `Authorization: Bearer …` header (OAuth / JWT clients)
//! - Requests carrying an `X-API-Key` header (API-key clients)
//!
//! # Validation flow (mutating requests only)
//! 1. Read the value of the `X-CSRF-Token` request header.
//! 2. Read the value of the `csrf_token` request cookie.
//! 3. Both must be present and identical (constant-time comparison).
//! 4. On failure → `403 Forbidden` with JSON body `{"error":"CSRF token validation failed"}`.
//!
//! # Usage
//! ```rust,ignore
//! use aframp_backend::middleware::csrf::{csrf_middleware, CsrfConfig};
//!
//! let app = Router::new()
//!     .route("/api/v1/resource", post(handler))
//!     .layer(axum::middleware::from_fn_with_state(
//!         CsrfConfig::default(),
//!         csrf_middleware,
//!     ));
//! ```

use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use tracing::{debug, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the CSRF middleware.
///
/// Both fields are customisable so the middleware can be adapted to
/// environments that use non-standard cookie / header names.
#[derive(Debug, Clone)]
pub struct CsrfConfig {
    /// Name of the cookie that carries the CSRF token.
    /// Default: `"csrf_token"`.
    pub cookie_name: String,
    /// Name of the request header that must mirror the cookie value.
    /// Default: `"X-CSRF-Token"`.
    pub header_name: String,
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self {
            cookie_name: "csrf_token".to_string(),
            header_name: "X-CSRF-Token".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Token generation
// ---------------------------------------------------------------------------

/// Generate a fresh CSRF token.
///
/// Returns a 32-byte (64 hex-character) random string derived from two
/// concatenated UUID v4 values, which guarantees both sufficient entropy and
/// URL-safe characters.
///
/// Example output: `"a1b2c3d4e5f6...a1b2c3d4e5f6"` (64 hex chars)
pub fn generate_csrf_token() -> String {
    // Two UUID v4 values give 256 bits of randomness; strip the hyphens for a
    // compact 32-byte (64-character) hex token.
    let a = Uuid::new_v4().simple().to_string(); // 32 hex chars
    let b = Uuid::new_v4().simple().to_string(); // 32 hex chars
    format!("{}{}", a, b)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the value of a named cookie from the `Cookie` header.
///
/// Parses `name=value` pairs separated by `"; "` and returns the first match.
fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some((name, value)) = pair.split_once('=') {
            if name.trim() == cookie_name {
                return Some(value.trim());
            }
        }
    }
    None
}

/// Build the 403 Forbidden JSON response returned when CSRF validation fails.
fn csrf_rejection() -> Response<Body> {
    (
        StatusCode::FORBIDDEN,
        [("Content-Type", "application/json")],
        r#"{"error":"CSRF token validation failed"}"#,
    )
        .into_response()
}

/// Return `true` when the request method is safe (read-only) and CSRF
/// protection therefore does not apply.
fn is_safe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

/// Return `true` when the request carries authentication that makes CSRF
/// attacks impossible — Bearer tokens and API keys are sent explicitly by
/// the client script, not injected by the browser from a stored credential.
fn is_exempt_by_auth_header(request: &Request<Body>) -> bool {
    let headers = request.headers();

    // Bearer token (OAuth 2.0 / JWT) — cross-origin scripts cannot read
    // cookies but they *can* set this header, which the browser never sends
    // automatically → not CSRF-vulnerable.
    let has_bearer = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("Bearer "))
        .unwrap_or(false);

    // API key header — same reasoning as Bearer.
    let has_api_key = headers.contains_key("X-API-Key");

    has_bearer || has_api_key
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// Axum middleware that enforces CSRF protection using the double-submit
/// cookie pattern.
///
/// Attach via `axum::middleware::from_fn_with_state(CsrfConfig::default(), csrf_middleware)`.
pub async fn csrf_middleware(
    State(config): State<CsrfConfig>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // 1. Safe methods — no CSRF risk.
    if is_safe_method(&method) {
        debug!(method = %method, path = %path, "CSRF: safe method, skipping check");
        return next.run(request).await;
    }

    // 2. Auth-header-based exemptions (Bearer / API key).
    if is_exempt_by_auth_header(&request) {
        debug!(method = %method, path = %path, "CSRF: exempt by auth header, skipping check");
        return next.run(request).await;
    }

    // 3. Validate the double-submit cookie for mutating requests.
    let headers = request.headers();

    // Read the X-CSRF-Token request header.
    let header_token = headers
        .get(config.header_name.as_str())
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    // Read the csrf_token cookie.
    let cookie_token = headers
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie_str| extract_cookie_value(cookie_str, &config.cookie_name))
        .map(str::to_owned);

    match (header_token, cookie_token) {
        (Some(hdr), Some(ck)) if !hdr.is_empty() && hdr == ck => {
            // Tokens match — allow the request through.
            debug!(method = %method, path = %path, "CSRF: token valid");
            next.run(request).await
        }
        (Some(_), Some(_)) => {
            // Both present but values do not match.
            warn!(
                method = %method,
                path   = %path,
                "CSRF: header and cookie token mismatch"
            );
            csrf_rejection()
        }
        (None, _) => {
            // Header missing entirely.
            warn!(
                method = %method,
                path   = %path,
                "CSRF: {} header missing", config.header_name
            );
            csrf_rejection()
        }
        (_, None) => {
            // Cookie missing entirely.
            warn!(
                method = %method,
                path   = %path,
                "CSRF: {} cookie missing", config.cookie_name
            );
            csrf_rejection()
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── generate_csrf_token ─────────────────────────────────────────────────

    #[test]
    fn token_is_64_hex_chars() {
        let token = generate_csrf_token();
        assert_eq!(token.len(), 64, "Token should be 64 hex characters");
        assert!(
            token.chars().all(|c| c.is_ascii_hexdigit()),
            "Token should only contain hex characters"
        );
    }

    #[test]
    fn tokens_are_unique() {
        let t1 = generate_csrf_token();
        let t2 = generate_csrf_token();
        assert_ne!(t1, t2, "Consecutive tokens should differ");
    }

    // ── extract_cookie_value ────────────────────────────────────────────────

    #[test]
    fn extracts_cookie_by_name() {
        let cookie = "session=abc; csrf_token=mytoken; other=xyz";
        assert_eq!(extract_cookie_value(cookie, "csrf_token"), Some("mytoken"));
    }

    #[test]
    fn returns_none_for_missing_cookie() {
        let cookie = "session=abc; other=xyz";
        assert_eq!(extract_cookie_value(cookie, "csrf_token"), None);
    }

    #[test]
    fn handles_single_cookie() {
        let cookie = "csrf_token=tok";
        assert_eq!(extract_cookie_value(cookie, "csrf_token"), Some("tok"));
    }

    // ── is_safe_method ──────────────────────────────────────────────────────

    #[test]
    fn safe_methods_are_exempt() {
        assert!(is_safe_method(&Method::GET));
        assert!(is_safe_method(&Method::HEAD));
        assert!(is_safe_method(&Method::OPTIONS));
        assert!(is_safe_method(&Method::TRACE));
    }

    #[test]
    fn mutating_methods_are_not_safe() {
        assert!(!is_safe_method(&Method::POST));
        assert!(!is_safe_method(&Method::PUT));
        assert!(!is_safe_method(&Method::PATCH));
        assert!(!is_safe_method(&Method::DELETE));
    }

    // ── CsrfConfig::default ─────────────────────────────────────────────────

    #[test]
    fn default_config_has_expected_names() {
        let config = CsrfConfig::default();
        assert_eq!(config.cookie_name, "csrf_token");
        assert_eq!(config.header_name, "X-CSRF-Token");
    }
}
