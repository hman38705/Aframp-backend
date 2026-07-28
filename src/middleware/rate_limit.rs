use crate::cache::RedisCache;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderValue, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, fs::File, net::SocketAddr, sync::Arc};
use tracing::{info, warn};
use uuid::Uuid;

/// Atomic sliding-window check-and-increment (Issue #727).
///
/// Combines ZREMRANGEBYSCORE + ZCARD + conditional ZADD/EXPIRE into a single
/// Lua script so the check and the increment happen as one atomic operation
/// on Redis — eliminating the race window that existed between the previous
/// separate "check" and "add" pipelines (two instances could both observe
/// `count < limit` and both add, letting `2x limit` requests through).
///
/// KEYS[1] = sorted-set key
/// ARGV[1] = now (ms)
/// ARGV[2] = window (ms)
/// ARGV[3] = limit
/// ARGV[4] = member id (unique per request)
/// ARGV[5] = window (seconds, for EXPIRE)
///
/// Returns { allowed (0|1), count } where `count` is the window occupancy
/// after this call (post-increment when allowed, current count when not).
static SLIDING_WINDOW_SCRIPT: Lazy<redis::Script> = Lazy::new(|| {
    redis::Script::new(
        r#"
        redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', tonumber(ARGV[1]) - tonumber(ARGV[2]))
        local count = redis.call('ZCARD', KEYS[1])
        local limit = tonumber(ARGV[3])
        if count < limit then
            redis.call('ZADD', KEYS[1], ARGV[1], ARGV[4])
            redis.call('EXPIRE', KEYS[1], ARGV[5])
            return {1, count + 1}
        else
            return {0, count}
        end
        "#,
    )
});

#[derive(Debug, Deserialize, Clone)]
pub struct LimitConfig {
    pub limit: i64,
    pub window: i64,
}

/// Endpoint sensitivity tier (Issue #726). Determines the default per-IP
/// limit applied when an endpoint entry doesn't set `per_ip` explicitly.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointTier {
    #[serde(rename = "CRITICAL")]
    Critical,
    #[serde(rename = "FINANCIAL")]
    Financial,
    #[serde(rename = "STANDARD")]
    Standard,
    #[serde(rename = "PUBLIC")]
    Public,
}

impl EndpointTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            EndpointTier::Critical => "CRITICAL",
            EndpointTier::Financial => "FINANCIAL",
            EndpointTier::Standard => "STANDARD",
            EndpointTier::Public => "PUBLIC",
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct EndpointLimits {
    pub per_ip: Option<LimitConfig>,
    pub per_wallet: Option<LimitConfig>,
    #[serde(default)]
    pub tier: Option<EndpointTier>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    pub endpoints: HashMap<String, EndpointLimits>,
    pub default: EndpointLimits,
    #[serde(default)]
    pub tiers: HashMap<EndpointTier, LimitConfig>,
}

impl RateLimitConfig {
    pub fn load(path: &str) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let config: RateLimitConfig = serde_yaml::from_reader(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }

    /// Resolves the limits for a given path, applying the endpoint's
    /// sensitivity tier (Issue #726) as the per_ip fallback when the
    /// matched entry (or the default) doesn't define per_ip explicitly.
    pub fn get_limits(&self, path: &str) -> EndpointLimits {
        let mut limits = self
            .endpoints
            .iter()
            .find(|(prefix, _)| path.starts_with(prefix.as_str()))
            .map(|(_, limits)| limits.clone())
            .unwrap_or_else(|| self.default.clone());

        if limits.per_ip.is_none() {
            limits.per_ip = limits
                .tier
                .and_then(|t| self.tiers.get(&t).cloned())
                .or_else(|| self.default.per_ip.clone());
        }

        limits
    }
}

#[derive(Clone)]
pub struct RateLimitState {
    pub cache: Arc<RedisCache>,
    pub config: Arc<RateLimitConfig>,
}

pub async fn rate_limit_middleware(
    State(state): State<RateLimitState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let path = req.uri().path().to_string();

    // 1. Bypass logic
    if path.starts_with("/health") {
        return Ok(next.run(req).await);
    }

    // Check admin bypass via header
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str == "Bearer admin-bypass-token"
                || req.headers().get("x-admin-token").is_some()
            {
                info!("Admin bypass triggered for rate limit on path: {}", path);
                return Ok(next.run(req).await);
            }
        }
    }

    let limits = state.config.get_limits(&path);
    let mut wallet_address = None;

    // 2. Extract Wallet from JWT if available
    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "default-development-secret-key-change-in-prod!".to_string());
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                use crate::api::auth::Claims; // using the Claims struct we built previously
                let mut validation = Validation::default();
                validation.validate_exp = true;
                validation.insecure_disable_signature_validation(); // Just decode safely for limits or properly validate if preferred, but verification is better. The actual auth middleware does strict verification. We'll just softly decode for identity.

                // Decode token securely
                if let Ok(token_data) = decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(secret.as_bytes()),
                    &Validation::default(),
                ) {
                    wallet_address = Some(token_data.claims.sub);
                }
            }
        }
    }

    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    let (redis_key, limit_conf) = if let Some(ref w) = wallet_address {
        if let Some(wl) = limits.per_wallet {
            (format!("rate_limit:wallet:{}:{}", w, path), wl)
        } else if let Some(il) = limits.per_ip {
            // Fallback to IP if no wallet limit exists
            (format!("rate_limit:ip:{}:{}", ip, path), il)
        } else {
            return Ok(next.run(req).await);
        }
    } else {
        if let Some(il) = limits.per_ip {
            (format!("rate_limit:ip:{}:{}", ip, path), il)
        } else {
            return Ok(next.run(req).await);
        }
    };

    // 3. Atomic sliding-window check-and-increment (Issue #727). Redis is
    // the single source of truth across all instances, so this is
    // distributed-safe; if Redis itself is unavailable, fail open (allow
    // the request) rather than 500ing every request in the fleet.
    let now_ms = Utc::now().timestamp_millis();
    let req_id = Uuid::new_v4().to_string();
    let reset_at = (now_ms / 1000) + limit_conf.window;

    let mut conn = match state.cache.get_connection().await {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Redis connection unavailable in rate_limit_middleware, failing open: {}",
                e
            );
            crate::middleware::rate_limit_metrics::record_redis_fallback("connection_error");
            return Ok(next.run(req).await);
        }
    };

    let (allowed, count): (i64, i64) = match SLIDING_WINDOW_SCRIPT
        .key(&redis_key)
        .arg(now_ms)
        .arg(limit_conf.window * 1000)
        .arg(limit_conf.limit)
        .arg(&req_id)
        .arg(limit_conf.window)
        .invoke_async(&mut *conn)
        .await
    {
        Ok(res) => res,
        Err(e) => {
            warn!(
                "Redis Lua script failed in rate_limit_middleware, failing open: {}",
                e
            );
            crate::middleware::rate_limit_metrics::record_redis_fallback("script_error");
            return Ok(next.run(req).await);
        }
    };

    let remaining = limit_conf.limit - count;
    let retry_after = limit_conf.window;

    if allowed == 0 {
        warn!(key = %redis_key, count = count, limit = limit_conf.limit, "Rate limit exceeded");

        // Emit metric for alert rule: aframp_rate_limit_breaches_total
        #[cfg(feature = "cache")]
        crate::metrics::alerting::rate_limit_breaches_total()
            .with_label_values(&[&path])
            .inc();

        let response_body = json!({
            "error": {
                "code": "RATE_LIMIT_EXCEEDED",
                "message": "Too many requests",
                "limit": limit_conf.limit,
                "remaining": 0,
                "retry_after": retry_after,
                "reset_at": chrono::DateTime::<Utc>::from_utc(
                    chrono::NaiveDateTime::from_timestamp_opt(reset_at, 0).unwrap_or_default(), Utc
                ).to_rfc3339()
            }
        });

        let mut res = (StatusCode::TOO_MANY_REQUESTS, Json(response_body)).into_response();
        res.headers_mut().insert(
            "X-RateLimit-Limit",
            HeaderValue::from_str(&limit_conf.limit.to_string()).unwrap(),
        );
        res.headers_mut()
            .insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));
        res.headers_mut().insert(
            "X-RateLimit-Reset",
            HeaderValue::from_str(&reset_at.to_string()).unwrap(),
        );
        res.headers_mut().insert(
            "X-RateLimit-Used",
            HeaderValue::from_str(&count.to_string()).unwrap(),
        );
        res.headers_mut().insert(
            "Retry-After",
            HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );

        return Err(res);
    }

    // The Lua script already recorded this request atomically — just
    // forward it and annotate the response with the resulting quota state.
    let mut res = next.run(req).await.into_response();

    // Inject successful Rate Limit headers
    res.headers_mut().insert(
        "X-RateLimit-Limit",
        HeaderValue::from_str(&limit_conf.limit.to_string()).unwrap(),
    );
    res.headers_mut().insert(
        "X-RateLimit-Remaining",
        HeaderValue::from_str(&remaining.max(0).to_string()).unwrap(),
    );
    res.headers_mut().insert(
        "X-RateLimit-Reset",
        HeaderValue::from_str(&reset_at.to_string()).unwrap(),
    );
    res.headers_mut().insert(
        "X-RateLimit-Used",
        HeaderValue::from_str(&count.to_string()).unwrap(),
    );

    Ok(res)
}
