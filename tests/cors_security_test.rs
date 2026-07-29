//! Integration tests for CORS and Security Headers middleware
//!
//! Tests the implementation of Issue #86 - CORS and Security Headers
//!
//! # Note on unwrap/expect usage
//! All `unwrap()` calls in this file are intentional test-fixture boilerplate:
//! building requests, driving `oneshot`, and reading expected response headers.
//! Panicking on failure is correct in tests — it produces a clear, immediate
//! error message. No production code paths are involved.

use axum::{
    body::Body,
    http::{Request, StatusCode, Method},
    Router,
    routing::get,
    response::IntoResponse,
};
use tower::ServiceExt;
use tower::ServiceBuilder;

// Import the middleware modules
use crate::middleware::cors::{cors_middleware, CorsConfig};
use crate::middleware::security::security_headers_middleware;

async fn test_handler() -> impl IntoResponse {
    "OK"
}

fn create_test_app() -> Router {
    Router::new()
        .route("/test", get(test_handler))
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn_with_state(
                    CorsConfig::from_env(),
                    cors_middleware,
                ))
                .layer(axum::middleware::from_fn(security_headers_middleware))
        )
}

#[tokio::test]
async fn test_cors_preflight_allowed_origin() {
    let app = create_test_app();
    
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/test")
        .header("Origin", "http://localhost:3000")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Content-Type")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    
    let headers = response.headers();
    assert_eq!(
        headers.get("Access-Control-Allow-Origin").unwrap(),
        "http://localhost:3000"
    );
    assert!(headers.contains_key("Access-Control-Allow-Methods"));
    assert!(headers.contains_key("Access-Control-Allow-Headers"));
    assert_eq!(
        headers.get("Access-Control-Allow-Credentials").unwrap(),
        "true"
    );
}

#[tokio::test]
async fn test_cors_preflight_disallowed_origin() {
    let app = create_test_app();
    
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/test")
        .header("Origin", "https://malicious.com")
        .header("Access-Control-Request-Method", "POST")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    
    let headers = response.headers();
    // Should not have CORS headers for disallowed origin
    assert!(!headers.contains_key("Access-Control-Allow-Origin"));
}

#[tokio::test]
async fn test_cors_simple_request_allowed_origin() {
    let app = create_test_app();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header("Origin", "http://localhost:3000")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let headers = response.headers();
    assert_eq!(
        headers.get("Access-Control-Allow-Origin").unwrap(),
        "http://localhost:3000"
    );
    assert_eq!(
        headers.get("Access-Control-Allow-Credentials").unwrap(),
        "true"
    );
}

#[tokio::test]
async fn test_security_headers_present() {
    let app = create_test_app();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let headers = response.headers();
    
    // Test security headers
    assert_eq!(headers.get("X-Frame-Options").unwrap(), "DENY");
    assert_eq!(headers.get("X-Content-Type-Options").unwrap(), "nosniff");
    assert_eq!(headers.get("X-XSS-Protection").unwrap(), "1; mode=block");
    assert_eq!(
        headers.get("Referrer-Policy").unwrap(),
        "strict-origin-when-cross-origin"
    );
    assert!(headers.contains_key("Permissions-Policy"));
    assert!(headers.contains_key("Content-Security-Policy"));
    assert_eq!(headers.get("Server").unwrap(), "Aframp API");
    
    // Ensure X-Powered-By is removed
    assert!(!headers.contains_key("X-Powered-By"));
}

#[tokio::test]
async fn test_hsts_not_added_in_development() {
    // Set development environment
    std::env::set_var("ENVIRONMENT", "development");
    
    let app = create_test_app();
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    let headers = response.headers();
    
    // HSTS should not be present in development
    assert!(!headers.contains_key("Strict-Transport-Security"));
}

#[tokio::test]
async fn test_cors_config_from_env() {
    // Test development environment
    std::env::set_var("ENVIRONMENT", "development");
    let config = CorsConfig::from_env();
    assert!(config.allowed_origins.contains(&"http://localhost:3000".to_string()));
    assert!(config.allow_credentials);
    
    // Test production environment
    std::env::set_var("ENVIRONMENT", "production");
    let config = CorsConfig::from_env();
    assert!(config.allowed_origins.contains(&"https://app.aframp.com".to_string()));
    assert!(!config.allowed_origins.contains(&"http://localhost:3000".to_string()));
}

#[tokio::test]
async fn test_custom_cors_origins() {
    // Test custom origins via environment variable
    std::env::set_var("CORS_ALLOWED_ORIGINS", "https://custom1.com,https://custom2.com");
    std::env::set_var("ENVIRONMENT", "production");
    
    let config = CorsConfig::from_env();
    assert!(config.allowed_origins.contains(&"https://custom1.com".to_string()));
    assert!(config.allowed_origins.contains(&"https://custom2.com".to_string()));
    assert!(config.allowed_origins.contains(&"https://app.aframp.com".to_string()));
}

// =============================================================================
// CSRF middleware tests (Issue #715)
// =============================================================================
//
// Each test builds a minimal Axum router with `csrf_middleware` applied, then
// drives it with `tower::ServiceExt::oneshot` and asserts the HTTP status.
//
// # Note on unwrap/expect usage
// Same rationale as the CORS tests above: panicking on a broken fixture gives
// an immediate, informative test failure with no production code involved.

use aframp_backend::middleware::csrf::{csrf_middleware, CsrfConfig};

/// Minimal handler used in all CSRF test apps — always returns 200 OK so that
/// any 403 we observe originates exclusively from the CSRF middleware.
async fn csrf_test_handler() -> impl IntoResponse {
    "OK"
}

/// Build a `Router` with the CSRF middleware applied to every route using the
/// default `CsrfConfig` (cookie = `csrf_token`, header = `X-CSRF-Token`).
fn create_csrf_test_app() -> Router {
    Router::new()
        .route("/protected", axum::routing::post(csrf_test_handler))
        .route("/protected", axum::routing::get(csrf_test_handler))
        .route("/protected", axum::routing::put(csrf_test_handler))
        .route("/protected", axum::routing::delete(csrf_test_handler))
        .layer(axum::middleware::from_fn_with_state(
            CsrfConfig::default(),
            csrf_middleware,
        ))
}

// ---------------------------------------------------------------------------
// Test: POST without any CSRF token is blocked (403)
// ---------------------------------------------------------------------------

/// A plain POST request with no `X-CSRF-Token` header and no `csrf_token`
/// cookie must be rejected with 403 Forbidden.
#[tokio::test]
async fn test_csrf_blocks_post_without_token() {
    let app = create_csrf_test_app();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/protected")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "POST without CSRF token should be rejected with 403"
    );
}

// ---------------------------------------------------------------------------
// Test: GET without any CSRF token is allowed (safe method)
// ---------------------------------------------------------------------------

/// GET is a safe (read-only) method. The CSRF middleware must pass it through
/// without requiring any token.
#[tokio::test]
async fn test_csrf_allows_get_without_token() {
    let app = create_csrf_test_app();

    let request = Request::builder()
        .method(Method::GET)
        .uri("/protected")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET without CSRF token should be allowed (safe method)"
    );
}

// ---------------------------------------------------------------------------
// Test: POST with Authorization: Bearer is exempt (JWT/OAuth flow)
// ---------------------------------------------------------------------------

/// Requests that carry `Authorization: Bearer <token>` are not CSRF-vulnerable
/// because the browser never attaches Bearer tokens automatically. The
/// middleware must let them through even without a CSRF token.
#[tokio::test]
async fn test_csrf_allows_bearer_post_without_token() {
    let app = create_csrf_test_app();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/protected")
        .header("Authorization", "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "POST with Authorization: Bearer should be exempt from CSRF check"
    );
}

// ---------------------------------------------------------------------------
// Test: POST with X-API-Key is exempt (API key auth flow)
// ---------------------------------------------------------------------------

/// Requests that carry an `X-API-Key` header are programmatic API calls that
/// are not subject to CSRF attacks. The middleware must pass them through.
#[tokio::test]
async fn test_csrf_allows_api_key_post_without_token() {
    let app = create_csrf_test_app();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/protected")
        .header("X-API-Key", "ak_live_supersecretapikey123")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "POST with X-API-Key should be exempt from CSRF check"
    );
}

// ---------------------------------------------------------------------------
// Test: POST with matching header + cookie passes through (valid CSRF)
// ---------------------------------------------------------------------------

/// The happy path: a POST request that carries a `X-CSRF-Token` header whose
/// value matches the `csrf_token` cookie must be allowed through (200 OK).
#[tokio::test]
async fn test_csrf_allows_post_with_matching_token() {
    let app = create_csrf_test_app();

    // Use a fixed token value — in production this would come from
    // `generate_csrf_token()`, but a predictable value is fine in tests.
    let token = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";

    let request = Request::builder()
        .method(Method::POST)
        .uri("/protected")
        // The header must carry the same value as the cookie.
        .header("X-CSRF-Token", token)
        .header("Cookie", format!("csrf_token={}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "POST with matching CSRF header+cookie should be allowed"
    );
}

// =============================================================================
// CORS hardening tests (Issue #716)
// =============================================================================
//
// These tests verify:
//   1. Vary: Origin is present on all CORS responses (cache-poisoning defence).
//   2. CORS_ALLOWED_ORIGINS containing '*' is rejected in staging / production.
//   3. Development is deliberately exempt from the wildcard check.
//
// Tests that call `validate_production_config` require the full set of env vars
// that the validator inspects.  We use the same pattern as the existing tests in
// config_validation.rs — `std::env::set_var` immediately before the call.
//
// NOTE: env-var tests are NOT isolated from each other when running in parallel.
// Cargo runs tests in the same process with the same env table.  If parallelism
// causes flakiness, run with `-- --test-threads=1`.

use aframp_backend::config_validation::validate_production_config;

// ---------------------------------------------------------------------------
// Helper: set the minimum env vars required by validate_production_config to
// pass all checks *other than* the one under test.  Returns a closure that
// callers can use to set the one variable they want to test.
// ---------------------------------------------------------------------------

/// Set env vars that satisfy every validate_production_config check for a
/// non-development environment so that only the CORS check is the subject of
/// the test.
///
/// `app_env` should be `"staging"` or `"production"`.
fn set_valid_non_dev_env(app_env: &str) {
    std::env::set_var("APP_ENV", app_env);
    std::env::set_var(
        "DATABASE_URL",
        "postgres://user:pass@host/db?sslmode=require",
    );
    std::env::set_var("REDIS_URL", "rediss://localhost:6379");
    std::env::set_var("STELLAR_NETWORK", "mainnet");
    std::env::set_var(
        "JWT_SECRET",
        "a-very-long-secret-that-is-at-least-32-chars-long",
    );
    std::env::set_var(
        "ENCRYPTION_KEY",
        "a-very-long-encryption-key-at-least-32-chars",
    );
    std::env::set_var("PAYSTACK_SECRET_KEY", "sk_live_realkey123456789");
    std::env::set_var("SYSTEM_WALLET_SECRET", "SREAL_STELLAR_SECRET_KEY_HERE_LONG");
    // Ensure disabling flags that would trigger other errors
    std::env::set_var("ENABLE_MOCK_PAYMENTS", "false");
    std::env::set_var("DEBUG_MODE", "false");
    // Do NOT set LOG_FORMAT=plain (would trigger log-format error)
    std::env::remove_var("LOG_FORMAT");
}

// ---------------------------------------------------------------------------
// Test 1: Vary: Origin is set on CORS preflight responses
// ---------------------------------------------------------------------------

/// After Issue #716, every CORS response (preflight and simple) must carry
/// `Vary: Origin` so that downstream caches never serve a cached response
/// from one origin to a different requesting origin.
#[tokio::test]
async fn test_vary_origin_header_present() {
    // Drive the test app with a preflight from an allowed development origin.
    std::env::set_var("ENVIRONMENT", "development");
    std::env::remove_var("CORS_ALLOWED_ORIGINS");

    let app = create_test_app();

    // --- preflight ---
    let preflight = Request::builder()
        .method(Method::OPTIONS)
        .uri("/test")
        .header("Origin", "http://localhost:3000")
        .header("Access-Control-Request-Method", "GET")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(preflight).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NO_CONTENT,
        "Preflight should return 204 No Content"
    );
    assert_eq!(
        response.headers().get("Vary").map(|v| v.to_str().unwrap()),
        Some("Origin"),
        "Vary: Origin must be present on preflight CORS response"
    );

    // --- simple GET (non-preflight) ---
    let simple = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header("Origin", "http://localhost:3000")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(simple).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("Vary").map(|v| v.to_str().unwrap()),
        Some("Origin"),
        "Vary: Origin must be present on simple (non-preflight) CORS response"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Wildcard '*' in CORS_ALLOWED_ORIGINS is rejected in staging
// ---------------------------------------------------------------------------

/// validate_production_config must return an error containing "wildcard" (or
/// the literal `'*'`) when APP_ENV=staging and CORS_ALLOWED_ORIGINS contains
/// the bare wildcard.
#[test]
fn test_wildcard_rejected_in_staging() {
    set_valid_non_dev_env("staging");
    std::env::set_var("CORS_ALLOWED_ORIGINS", "*");

    let result = validate_production_config();

    assert!(
        result.is_err(),
        "validate_production_config must fail when CORS_ALLOWED_ORIGINS='*' in staging"
    );
    let err = result.unwrap_err();
    assert!(
        err.errors
            .iter()
            .any(|e| e.contains('*') || e.to_lowercase().contains("wildcard") || e.to_lowercase().contains("cors")),
        "Error list must mention the CORS wildcard problem; got: {:?}",
        err.errors
    );

    // Cleanup
    std::env::remove_var("CORS_ALLOWED_ORIGINS");
}

// ---------------------------------------------------------------------------
// Test 3: Wildcard '*' in CORS_ALLOWED_ORIGINS is rejected in production
// ---------------------------------------------------------------------------

/// Same as the staging test but for APP_ENV=production.
#[test]
fn test_wildcard_rejected_in_production() {
    set_valid_non_dev_env("production");
    std::env::set_var("CORS_ALLOWED_ORIGINS", "*");

    let result = validate_production_config();

    assert!(
        result.is_err(),
        "validate_production_config must fail when CORS_ALLOWED_ORIGINS='*' in production"
    );
    let err = result.unwrap_err();
    assert!(
        err.errors
            .iter()
            .any(|e| e.contains('*') || e.to_lowercase().contains("wildcard") || e.to_lowercase().contains("cors")),
        "Error list must mention the CORS wildcard problem; got: {:?}",
        err.errors
    );

    // Cleanup
    std::env::remove_var("CORS_ALLOWED_ORIGINS");
}

// ---------------------------------------------------------------------------
// Test 4: Wildcard '*' is allowed in development (dev is exempt)
// ---------------------------------------------------------------------------

/// Development environments are exempt from the wildcard restriction so that
/// local tooling (Storybook, proxies, etc.) can work without explicit origins.
/// validate_production_config must NOT push a CORS error when APP_ENV=development.
#[test]
fn test_wildcard_allowed_in_development() {
    std::env::set_var("APP_ENV", "development");
    std::env::set_var("DATABASE_URL", "postgres://localhost/test");
    // Short JWT secret is fine here — we are only checking that the CORS
    // wildcard does NOT produce an error in development.
    std::env::set_var("JWT_SECRET", "a-very-long-secret-at-least-32-chars!!");
    std::env::set_var("CORS_ALLOWED_ORIGINS", "*");

    let result = validate_production_config();

    // In development the CORS wildcard check is skipped (is_non_dev == false).
    // The result might still be Ok or Err for unrelated reasons; what matters
    // is that no CORS wildcard error is present.
    match result {
        Ok(_) => { /* no errors at all — definitely fine */ }
        Err(err) => {
            let has_cors_wildcard_error = err
                .errors
                .iter()
                .any(|e| (e.contains('*') || e.to_lowercase().contains("wildcard")) && e.to_lowercase().contains("cors"));
            assert!(
                !has_cors_wildcard_error,
                "Development should be exempt from the CORS wildcard check; got: {:?}",
                err.errors
            );
        }
    }

    // Cleanup
    std::env::remove_var("CORS_ALLOWED_ORIGINS");
}
