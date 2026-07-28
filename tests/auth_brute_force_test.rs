//! Integration tests for auth brute-force rate-limit middleware (Issue #722).
//!
//! Requires a running Redis instance.
//! Run with: cargo test --features cache auth_brute_force -- --ignored

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    response::IntoResponse,
    routing::post,
    Router,
};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a test router with the auth rate-limit middleware applied.
///
/// Each test uses a unique IP key prefix by injecting a fresh config so tests
/// don't interfere with each other.
fn build_router_with_config(
    state: aframp_backend::middleware::auth_rate_limit::AuthRateLimitState,
) -> Router {
    Router::new()
        .route("/auth/login", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            aframp_backend::middleware::auth_rate_limit::auth_rate_limit_middleware,
        ))
        // ConnectInfo is required by the middleware
        .into_make_service_with_connect_info::<SocketAddr>()
        .into_inner()
}

fn login_request(ip: &str, account: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("x-forwarded-for", ip);

    if let Some(acc) = account {
        builder = builder.header("X-Account-Id", acc);
    }

    builder.body(Body::empty()).unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Requests within the window should be allowed.
#[tokio::test]
#[ignore]
async fn auth_rate_limit_allows_within_window() {
    use aframp_backend::cache::{init_cache_pool, CacheConfig};
    use aframp_backend::middleware::auth_rate_limit::{AuthRateLimitConfig, AuthRateLimitState};

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .expect("Redis init failed");

    let config = AuthRateLimitConfig {
        max_attempts_per_window: 5,
        window_secs: 60,
        lockout_threshold: 10,
        lockout_secs: 60,
        account_id_header: "X-Account-Id".to_string(),
    };
    let state = AuthRateLimitState::new(Arc::new(pool), config);
    let router = Router::new()
        .route("/auth/login", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            aframp_backend::middleware::auth_rate_limit::auth_rate_limit_middleware,
        ));

    let unique_ip = format!("10.0.{}.1", rand_u8());
    for _ in 0..5 {
        let req = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("x-forwarded-for", &unique_ip)
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

/// After exceeding the IP limit, subsequent requests should return 429 with Retry-After.
#[tokio::test]
#[ignore]
async fn auth_rate_limit_blocks_after_ip_limit_exceeded() {
    use aframp_backend::cache::{init_cache_pool, CacheConfig};
    use aframp_backend::middleware::auth_rate_limit::{AuthRateLimitConfig, AuthRateLimitState};

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .expect("Redis init failed");

    let config = AuthRateLimitConfig {
        max_attempts_per_window: 3,
        window_secs: 60,
        lockout_threshold: 50, // high, so lockout doesn't fire first
        lockout_secs: 60,
        account_id_header: "X-Account-Id".to_string(),
    };
    let state = AuthRateLimitState::new(Arc::new(pool), config);
    let router = Router::new()
        .route("/auth/login", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            aframp_backend::middleware::auth_rate_limit::auth_rate_limit_middleware,
        ));

    // Use a unique IP per test run to avoid cross-test pollution
    let unique_ip = format!("10.1.{}.{}", rand_u8(), rand_u8());

    // First 3 requests allowed
    for _ in 0..3 {
        let req = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("x-forwarded-for", &unique_ip)
            .body(Body::empty())
            .unwrap();
        let _ = router.clone().oneshot(req).await.unwrap();
    }

    // 4th request over the limit
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("x-forwarded-for", &unique_ip)
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp.headers().get("Retry-After").is_some());

    // Verify response body contains the expected error code
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["error"]["code"], "AUTH_RATE_LIMIT_EXCEEDED");
}

/// After `lockout_threshold` failures recorded for an account, subsequent
/// requests for that account should return 429 ACCOUNT_LOCKED_OUT.
#[tokio::test]
#[ignore]
async fn auth_lockout_triggers_after_threshold() {
    use aframp_backend::cache::{init_cache_pool, CacheConfig};
    use aframp_backend::middleware::auth_rate_limit::{
        record_auth_failure, AuthRateLimitConfig, AuthRateLimitState,
    };

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .expect("Redis init failed");
    let pool = Arc::new(pool);

    let config = AuthRateLimitConfig {
        max_attempts_per_window: 100, // high IP limit so IP doesn't block first
        window_secs: 60,
        lockout_threshold: 3,
        lockout_secs: 60,
        account_id_header: "X-Account-Id".to_string(),
    };

    // Record 3 failures for the account
    let account = format!("test_lockout_{}", rand_u8());
    for _ in 0..3 {
        record_auth_failure(&pool, &account, &config)
            .await
            .expect("record failure");
    }

    // Now a request with this account header should be locked out
    let state = AuthRateLimitState::new(pool, config);
    let router = Router::new()
        .route("/auth/login", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            aframp_backend::middleware::auth_rate_limit::auth_rate_limit_middleware,
        ));

    let unique_ip = format!("10.2.{}.1", rand_u8());
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("x-forwarded-for", &unique_ip)
        .header("X-Account-Id", &account)
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["error"]["code"], "ACCOUNT_LOCKED_OUT");
    assert!(resp.headers().get("Retry-After").is_none()); // checked via original resp - re-check below
    assert!(json["error"]["retry_after_secs"].as_i64().unwrap() > 0);
}

/// After a successful login, the failure counter and lockout should clear.
#[tokio::test]
#[ignore]
async fn auth_success_clears_lockout() {
    use aframp_backend::cache::{init_cache_pool, CacheConfig};
    use aframp_backend::middleware::auth_rate_limit::{
        record_auth_failure, record_auth_success, AuthRateLimitConfig, AuthRateLimitState,
    };

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .expect("Redis init failed");
    let pool = Arc::new(pool);

    let config = AuthRateLimitConfig {
        max_attempts_per_window: 100,
        window_secs: 60,
        lockout_threshold: 2,
        lockout_secs: 60,
        account_id_header: "X-Account-Id".to_string(),
    };

    let account = format!("test_success_clear_{}", rand_u8());

    // Record 2 failures to trigger lockout
    for _ in 0..2 {
        record_auth_failure(&pool, &account, &config)
            .await
            .unwrap();
    }

    // Clear on success
    record_auth_success(&pool, &account).await.unwrap();

    // Now requests for this account should be allowed
    let state = AuthRateLimitState::new(pool, config);
    let router = Router::new()
        .route("/auth/login", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            aframp_backend::middleware::auth_rate_limit::auth_rate_limit_middleware,
        ));

    let unique_ip = format!("10.3.{}.1", rand_u8());
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("x-forwarded-for", &unique_ip)
        .header("X-Account-Id", &account)
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn rand_u8() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
        % 256) as u8
}
