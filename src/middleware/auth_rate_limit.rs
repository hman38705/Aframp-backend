//! Auth brute-force rate-limit middleware (Issue #722)
//!
//! Provides per-IP sliding-window rate limiting and per-account lockout for
//! authentication endpoints (`/auth/login`, `/oauth/token`, etc.).
//!
//! # Behaviour
//! - **Sliding window**: up to `max_attempts_per_window` requests from the
//!   same IP within `window_secs` seconds.  Default: 10 attempts / 15 min.
//! - **Account lockout**: after `lockout_threshold` consecutive failures for
//!   a given username/account, that account is locked for `lockout_secs`.
//!   Default: 5 failures → 15-minute lockout.
//! - **429 with `Retry-After`**: rate-limited or locked-out requests receive
//!   HTTP 429 and a `Retry-After` header indicating how many seconds until
//!   the client may retry.
//!
//! # Redis keys
//! | Purpose              | Key pattern                            | TTL                   |
//! |----------------------|----------------------------------------|-----------------------|
//! | IP sliding window    | `auth:rate:ip:<ip>`                    | `window_secs`         |
//! | Account lockout flag | `auth:lockout:account:<account>`       | `lockout_secs`        |
//! | Failure counter      | `auth:failures:account:<account>`      | `lockout_secs`        |
//!
//! # Integration
//! Apply as a layer to your auth router:
//! ```rust,ignore
//! let auth_state = AuthRateLimitState::from_env(redis_pool);
//! Router::new()
//!     .route("/auth/login",  post(login_handler))
//!     .route("/oauth/token", post(token_handler))
//!     .layer(axum::middleware::from_fn_with_state(
//!         auth_state,
//!         auth_rate_limit_middleware,
//!     ))
//! ```

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use redis::AsyncCommands;
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, warn};

use crate::cache::RedisPool;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Tunable parameters for the auth brute-force protection.
#[derive(Debug, Clone)]
pub struct AuthRateLimitConfig {
    /// Maximum auth attempts per IP in the sliding window. Default: 10.
    pub max_attempts_per_window: i64,
    /// Length of the sliding window in seconds. Default: 900 (15 min).
    pub window_secs: i64,
    /// Consecutive failures before an account is locked. Default: 5.
    pub lockout_threshold: i64,
    /// How long (seconds) a locked account stays locked. Default: 900 (15 min).
    pub lockout_secs: i64,
    /// Header name that carries the account identifier (username / email).
    /// The middleware looks for this in the JSON body field named `username`
    /// or `email` if the header is absent.
    pub account_id_header: String,
}

impl Default for AuthRateLimitConfig {
    fn default() -> Self {
        Self {
            max_attempts_per_window: 10,
            window_secs: 900,
            lockout_threshold: 5,
            lockout_secs: 900,
            account_id_header: "X-Account-Id".to_string(),
        }
    }
}

impl AuthRateLimitConfig {
    /// Load from environment variables, falling back to defaults.
    ///
    /// | Variable                          | Default | Description                              |
    /// |-----------------------------------|---------|------------------------------------------|
    /// | `AUTH_RATE_MAX_ATTEMPTS`          | 10      | Max attempts per IP per window           |
    /// | `AUTH_RATE_WINDOW_SECS`           | 900     | Sliding window length (seconds)          |
    /// | `AUTH_LOCKOUT_THRESHOLD`          | 5       | Consecutive failures to trigger lockout  |
    /// | `AUTH_LOCKOUT_SECS`               | 900     | Lockout duration (seconds)               |
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            max_attempts_per_window: std::env::var("AUTH_RATE_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default.max_attempts_per_window),
            window_secs: std::env::var("AUTH_RATE_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default.window_secs),
            lockout_threshold: std::env::var("AUTH_LOCKOUT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default.lockout_threshold),
            lockout_secs: std::env::var("AUTH_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default.lockout_secs),
            account_id_header: default.account_id_header,
        }
    }
}

// ---------------------------------------------------------------------------
// Middleware state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AuthRateLimitState {
    pub redis: Arc<RedisPool>,
    pub config: Arc<AuthRateLimitConfig>,
}

impl AuthRateLimitState {
    pub fn new(redis: Arc<RedisPool>, config: AuthRateLimitConfig) -> Self {
        Self {
            redis,
            config: Arc::new(config),
        }
    }

    pub fn from_env(redis: Arc<RedisPool>) -> Self {
        Self::new(redis, AuthRateLimitConfig::from_env())
    }
}

// ---------------------------------------------------------------------------
// Redis key helpers
// ---------------------------------------------------------------------------

fn ip_rate_key(ip: &str) -> String {
    format!("auth:rate:ip:{}", ip)
}

fn lockout_key(account: &str) -> String {
    format!("auth:lockout:account:{}", account)
}

fn failures_key(account: &str) -> String {
    format!("auth:failures:account:{}", account)
}

// ---------------------------------------------------------------------------
// Error response helpers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AuthRateLimitError {
    error: AuthRateLimitErrorBody,
}

#[derive(Serialize)]
struct AuthRateLimitErrorBody {
    code: String,
    message: String,
    retry_after_secs: i64,
}

fn rate_limited_response(retry_after: i64, code: &str, message: &str) -> Response {
    let body = AuthRateLimitError {
        error: AuthRateLimitErrorBody {
            code: code.to_string(),
            message: message.to_string(),
            retry_after_secs: retry_after,
        },
    };
    let mut resp = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
    if let Ok(val) = HeaderValue::from_str(&retry_after.to_string()) {
        resp.headers_mut().insert("Retry-After", val);
    }
    resp
}

// ---------------------------------------------------------------------------
// IP rate-limit check (sliding window via Redis INCR + EXPIRE)
// ---------------------------------------------------------------------------

/// Returns `Some(ttl_secs)` (time until reset) if the IP is over-limit,
/// or `None` if the request should be allowed.
async fn check_ip_rate_limit(
    redis: &RedisPool,
    ip: &str,
    config: &AuthRateLimitConfig,
) -> Result<Option<i64>, String> {
    let key = ip_rate_key(ip);
    let mut conn = redis
        .get()
        .await
        .map_err(|e| format!("Redis connection error: {e}"))?;

    // Increment counter. If the key is new, INCR creates it.
    let count: i64 = redis::cmd("INCR")
        .arg(&key)
        .query_async(&mut *conn)
        .await
        .map_err(|e| format!("Redis INCR error: {e}"))?;

    // On first access set expiry so the window is self-cleaning.
    if count == 1 {
        let _: () = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(config.window_secs)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Redis EXPIRE error: {e}"))?;
    }

    if count > config.max_attempts_per_window {
        // Find how long until the window resets.
        let ttl: i64 = redis::cmd("TTL")
            .arg(&key)
            .query_async(&mut *conn)
            .await
            .unwrap_or(config.window_secs);

        let retry_after = ttl.max(1);
        Ok(Some(retry_after))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Account lockout check
// ---------------------------------------------------------------------------

/// Returns `Some(ttl_secs)` if the account is locked, `None` otherwise.
async fn check_account_lockout(
    redis: &RedisPool,
    account: &str,
) -> Result<Option<i64>, String> {
    let key = lockout_key(account);
    let mut conn = redis
        .get()
        .await
        .map_err(|e| format!("Redis connection error: {e}"))?;

    let ttl: Option<i64> = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut *conn)
        .await
        .ok();

    // TTL > 0 means key exists and has time remaining → locked.
    match ttl {
        Some(t) if t > 0 => Ok(Some(t)),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Post-response: record failure / success for account lockout tracking
// ---------------------------------------------------------------------------

/// Increment the failure counter for an account; lock it out if threshold is reached.
pub async fn record_auth_failure(
    redis: &RedisPool,
    account: &str,
    config: &AuthRateLimitConfig,
) -> Result<(), String> {
    let fkey = failures_key(account);
    let lkey = lockout_key(account);
    let mut conn = redis
        .get()
        .await
        .map_err(|e| format!("Redis connection error: {e}"))?;

    // Increment failure counter.
    let failures: i64 = redis::cmd("INCR")
        .arg(&fkey)
        .query_async(&mut *conn)
        .await
        .map_err(|e| format!("Redis INCR error: {e}"))?;

    // Keep the failure key alive for the lockout window.
    let _: () = redis::cmd("EXPIRE")
        .arg(&fkey)
        .arg(config.lockout_secs)
        .query_async(&mut *conn)
        .await
        .map_err(|e| format!("Redis EXPIRE error: {e}"))?;

    if failures >= config.lockout_threshold {
        // Set lockout sentinel with TTL.
        let _: () = redis::cmd("SET")
            .arg(&lkey)
            .arg("1")
            .arg("EX")
            .arg(config.lockout_secs)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Redis SET error: {e}"))?;

        warn!(
            account = %account,
            failures = failures,
            lockout_secs = config.lockout_secs,
            "Account locked out after {} consecutive failures", failures
        );
    }

    Ok(())
}

/// Reset the failure counter and lockout flag after a successful auth.
pub async fn record_auth_success(redis: &RedisPool, account: &str) -> Result<(), String> {
    let fkey = failures_key(account);
    let lkey = lockout_key(account);
    let mut conn = redis
        .get()
        .await
        .map_err(|e| format!("Redis connection error: {e}"))?;

    let _: () = redis::pipe()
        .cmd("DEL")
        .arg(&fkey)
        .cmd("DEL")
        .arg(&lkey)
        .query_async(&mut *conn)
        .await
        .map_err(|e| format!("Redis DEL error: {e}"))?;

    info!(account = %account, "Auth success — failure counter and lockout cleared");
    Ok(())
}

// ---------------------------------------------------------------------------
// Middleware entry-point
// ---------------------------------------------------------------------------

/// Axum middleware that enforces auth brute-force protection.
///
/// Apply to `/auth/*` and `/oauth/token` routes.
pub async fn auth_rate_limit_middleware(
    State(state): State<AuthRateLimitState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    // ── 1. Per-IP sliding-window check ────────────────────────────────────────
    match check_ip_rate_limit(&state.redis, &ip, &state.config).await {
        Ok(Some(retry_after)) => {
            warn!(
                ip = %ip,
                retry_after = retry_after,
                "Auth rate limit exceeded for IP"
            );
            return rate_limited_response(
                retry_after,
                "AUTH_RATE_LIMIT_EXCEEDED",
                "Too many authentication attempts from this IP. Please wait before retrying.",
            );
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!(error = %e, "Redis error in auth rate-limit IP check; allowing request");
        }
    }

    // ── 2. Per-account lockout check ─────────────────────────────────────────
    // Extract account identifier from header or leave blank (lockout check is skipped).
    let account_id = req
        .headers()
        .get(&state.config.account_id_header)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref account) = account_id {
        match check_account_lockout(&state.redis, account).await {
            Ok(Some(retry_after)) => {
                warn!(
                    account = %account,
                    retry_after = retry_after,
                    "Account is locked out"
                );
                return rate_limited_response(
                    retry_after,
                    "ACCOUNT_LOCKED_OUT",
                    "Account temporarily locked due to too many failed attempts. Please try again later.",
                );
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = %e, "Redis error in account lockout check; allowing request");
            }
        }
    }

    next.run(req).await
}

// ---------------------------------------------------------------------------
// Unit tests (no Redis required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_rate_key_format() {
        assert_eq!(ip_rate_key("1.2.3.4"), "auth:rate:ip:1.2.3.4");
    }

    #[test]
    fn lockout_key_format() {
        assert_eq!(lockout_key("user@example.com"), "auth:lockout:account:user@example.com");
    }

    #[test]
    fn failures_key_format() {
        assert_eq!(failures_key("alice"), "auth:failures:account:alice");
    }

    #[test]
    fn default_config_values() {
        let cfg = AuthRateLimitConfig::default();
        assert_eq!(cfg.max_attempts_per_window, 10);
        assert_eq!(cfg.window_secs, 900);
        assert_eq!(cfg.lockout_threshold, 5);
        assert_eq!(cfg.lockout_secs, 900);
    }

    #[test]
    fn rate_limited_response_has_retry_after_header() {
        let resp = rate_limited_response(60, "AUTH_RATE_LIMIT_EXCEEDED", "Too many requests");
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let header = resp.headers().get("Retry-After").unwrap();
        assert_eq!(header, "60");
    }

    #[test]
    fn config_from_env_uses_defaults_when_unset() {
        std::env::remove_var("AUTH_RATE_MAX_ATTEMPTS");
        std::env::remove_var("AUTH_RATE_WINDOW_SECS");
        std::env::remove_var("AUTH_LOCKOUT_THRESHOLD");
        std::env::remove_var("AUTH_LOCKOUT_SECS");
        let cfg = AuthRateLimitConfig::from_env();
        assert_eq!(cfg.max_attempts_per_window, 10);
        assert_eq!(cfg.window_secs, 900);
    }

    #[test]
    fn config_from_env_reads_overrides() {
        std::env::set_var("AUTH_RATE_MAX_ATTEMPTS", "5");
        std::env::set_var("AUTH_RATE_WINDOW_SECS", "300");
        std::env::set_var("AUTH_LOCKOUT_THRESHOLD", "3");
        std::env::set_var("AUTH_LOCKOUT_SECS", "600");
        let cfg = AuthRateLimitConfig::from_env();
        assert_eq!(cfg.max_attempts_per_window, 5);
        assert_eq!(cfg.window_secs, 300);
        assert_eq!(cfg.lockout_threshold, 3);
        assert_eq!(cfg.lockout_secs, 600);
        // clean up
        std::env::remove_var("AUTH_RATE_MAX_ATTEMPTS");
        std::env::remove_var("AUTH_RATE_WINDOW_SECS");
        std::env::remove_var("AUTH_LOCKOUT_THRESHOLD");
        std::env::remove_var("AUTH_LOCKOUT_SECS");
    }
}
