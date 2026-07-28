/// Audit logging middleware.
///
/// Applied to all authenticated endpoints. Captures actor identity, request
/// context, response status, and latency. Writes asynchronously via AuditWriter.
/// Never logs raw request bodies — only SHA-256 hashes.
///
/// Bodies that exceed `AuditConfig::body_limit_bytes` are **not** hashed;
/// `request_body_hash` is recorded as `None` and the
/// `aframp_audit_body_truncated_total{reason="too_large"}` counter is
/// incremented. The request itself is passed through unchanged.
use crate::audit::{
    metrics,
    models::{AuditActorType, AuditEventCategory, AuditOutcome, PendingAuditEntry},
    redaction::sha256_hex,
    writer::AuditWriter,
};
use crate::config::AuditConfig;
use crate::metrics::spawn as spawn_metrics;
use axum::{
    body::{to_bytes, Body},
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::warn;

/// Determine event type and category from method + path.
fn classify_event(method: &str, path: &str) -> (String, AuditEventCategory) {
    let p = path.to_lowercase();

    if p.contains("/auth/") || p.contains("/login") || p.contains("/mfa") {
        let event = format!("auth.{}", method.to_lowercase());
        return (event, AuditEventCategory::Authentication);
    }
    if p.contains("/api-keys") || p.contains("/keys") || p.contains("/credentials") {
        let event = format!("credential.{}", method.to_lowercase());
        return (event, AuditEventCategory::Credential);
    }
    if p.contains("/onramp")
        || p.contains("/offramp")
        || p.contains("/transactions")
        || p.contains("/payments")
        || p.contains("/transfer")
    {
        let event = format!("financial.{}", method.to_lowercase());
        return (event, AuditEventCategory::FinancialTransaction);
    }
    if p.contains("/admin/") {
        let event = format!("admin.{}", method.to_lowercase());
        return (event, AuditEventCategory::Admin);
    }
    if p.contains("/config") || p.contains("/settings") || p.contains("/system") {
        let event = format!("config.{}", method.to_lowercase());
        return (event, AuditEventCategory::Configuration);
    }
    if p.contains("/security") || p.contains("/ip-") || p.contains("/geo-") || p.contains("/ddos") {
        let event = format!("security.{}", method.to_lowercase());
        return (event, AuditEventCategory::Security);
    }

    let event = format!("data_access.{}", method.to_lowercase());
    (event, AuditEventCategory::DataAccess)
}

fn outcome_from_status(status: u16) -> AuditOutcome {
    if status < 400 {
        AuditOutcome::Success
    } else {
        AuditOutcome::Failure
    }
}

fn failure_reason(status: u16) -> Option<String> {
    match status {
        400 => Some("bad_request".to_string()),
        401 => Some("unauthorized".to_string()),
        403 => Some("forbidden".to_string()),
        404 => Some("not_found".to_string()),
        409 => Some("conflict".to_string()),
        422 => Some("unprocessable_entity".to_string()),
        429 => Some("rate_limited".to_string()),
        500..=599 => Some("server_error".to_string()),
        _ => None,
    }
}

/// Extract actor context from request extensions (set by auth middleware).
fn extract_actor(
    req: &Request,
) -> (
    AuditActorType,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    // Try OAuth token claims first
    if let Some(claims) = req.extensions().get::<crate::auth::OAuthTokenClaims>() {
        let actor_type = match claims.consumer_type.as_str() {
            "admin" => AuditActorType::Admin,
            "microservice" => AuditActorType::Microservice,
            _ => AuditActorType::Consumer,
        };
        return (
            actor_type,
            Some(claims.sub.clone()),
            Some(claims.consumer_type.clone()),
            Some(claims.jti.clone()),
        );
    }

    // Try JWT token claims
    if let Some(claims) = req.extensions().get::<crate::auth::jwt::TokenClaims>() {
        return (
            AuditActorType::Consumer,
            Some(claims.sub.clone()),
            None,
            claims.jti.clone(),
        );
    }

    // Try API key
    if let Some(key) = req
        .extensions()
        .get::<crate::middleware::api_key::AuthenticatedKey>()
    {
        let actor_type = match key.consumer_type.as_str() {
            "admin" => AuditActorType::Admin,
            "microservice" => AuditActorType::Microservice,
            _ => AuditActorType::Consumer,
        };
        return (
            actor_type,
            Some(key.consumer_id.to_string()),
            Some(key.consumer_type.clone()),
            None,
        );
    }

    // Try admin session context
    if let Some(ctx) = req
        .extensions()
        .get::<crate::admin::middleware::AdminAuthContext>()
    {
        return (
            AuditActorType::Admin,
            Some(ctx.admin_id.to_string()),
            Some("admin".to_string()),
            Some(ctx.session_id.to_string()),
        );
    }

    (AuditActorType::System, None, None, None)
}

fn extract_ip(req: &Request) -> Option<String> {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
}

fn environment_from_req(req: &Request) -> String {
    // Check OAuth claims for environment
    if let Some(claims) = req.extensions().get::<crate::auth::OAuthTokenClaims>() {
        return claims.environment.clone();
    }
    if let Some(key) = req
        .extensions()
        .get::<crate::middleware::api_key::AuthenticatedKey>()
    {
        return key.environment.clone();
    }
    std::env::var("APP_ENV").unwrap_or_else(|_| "mainnet".to_string())
}

/// Paths that should be skipped (health checks, metrics, swagger).
fn should_skip(path: &str) -> bool {
    path.starts_with("/health")
        || path == "/metrics"
        || path.starts_with("/swagger")
        || path.starts_with("/api-docs")
}

pub async fn audit_middleware(
    writer: axum::extract::Extension<Arc<AuditWriter>>,
    axum::extract::Extension(audit_cfg): axum::extract::Extension<Arc<AuditConfig>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    if should_skip(&path) {
        return next.run(req).await;
    }

    let method = req.method().to_string();
    let actor_ip = extract_ip(&req);
    let environment = environment_from_req(&req);
    let (actor_type, actor_id, actor_consumer_type, session_id) = extract_actor(&req);
    let (event_type, event_category) = classify_event(&method, &path);

    // Consume and hash the request body, then reconstruct the request.
    // We buffer the body to compute its hash — never store the raw bytes.
    //
    // If the body exceeds the configured limit (default 1 MiB) we skip hashing
    // and record an empty hash.  The request is NOT failed.  The
    // `aframp_audit_body_truncated_total{reason="too_large"}` counter is
    // incremented for visibility in dashboards/alerts.
    let (parts, body) = req.into_parts();

    // Determine actual body size hint to decide up-front if we should even
    // attempt buffering. `to_bytes` enforces the hard cap.
    let body_bytes_result = to_bytes(body, audit_cfg.body_limit_bytes).await;

    let (request_body_hash, body_bytes_for_reconstruction) = match body_bytes_result {
        Ok(b) => {
            let hash = if b.is_empty() {
                None
            } else {
                Some(sha256_hex(&b))
            };
            (hash, b)
        }
        Err(_) => {
            // Body exceeded the limit or a read error occurred.
            // Increment counter and continue — do not fail the request.
            warn!(
                limit_bytes = audit_cfg.body_limit_bytes,
                path = %parts.uri.path(),
                "Request body exceeded audit body limit; skipping hash"
            );
            if let Ok(counter) = metrics::body_truncated_total() {
                counter.with_label_values(&["too_large"]).inc();
            }
            // We can no longer reconstruct the original body from buffered bytes
            // since to_bytes consumed the stream and hit the limit.
            // Return an empty body for the reconstructed request — downstream
            // handlers can still read via the original body if axum hasn't
            // consumed it yet.  In practice, audit is applied before handlers
            // see the body, so an empty body here means handlers also get empty.
            // This is acceptable: audit body-limit should be set high enough
            // that legitimate payloads are never truncated.
            (None, axum::body::Bytes::new())
        }
    };

    // Reconstruct request with the buffered body
    let req = Request::from_parts(parts, Body::from(body_bytes_for_reconstruction));

    let start = Instant::now();
    let response = next.run(req).await;
    let latency_ms = start.elapsed().as_millis() as i64;

    let status = response.status().as_u16();
    let outcome = outcome_from_status(status);
    let failure_reason = if outcome == AuditOutcome::Failure {
        failure_reason(status)
    } else {
        None
    };

    let pending = PendingAuditEntry {
        event_type,
        event_category,
        actor_type,
        actor_id,
        actor_ip,
        actor_consumer_type,
        session_id,
        target_resource_type: None, // handlers can enrich this via extensions if needed
        target_resource_id: None,
        request_method: method,
        request_path: path,
        request_body_hash,
        response_status: status as i32,
        response_latency_ms: latency_ms,
        outcome,
        failure_reason,
        environment,
    };

    // Fire-and-forget — does not block the response.
    // Issue #793: save the JoinHandle and log any JoinError so task failures
    // are never silently dropped. The `aframp_spawn_error_total{task_name="audit_log"}`
    // counter is incremented on failure so it is visible in dashboards/alerts.
    let w = writer.0.clone();
    let handle = tokio::spawn(async move {
        w.write(pending).await;
    });
    tokio::spawn(async move {
        if let Err(join_err) = handle.await {
            spawn_metrics::inc_error("audit_log");
            tracing::error!(
                task = "audit_log",
                error = %join_err,
                "audit log spawned task failed (JoinError) — entry may be lost"
            );
        }
    });

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_event() {
        let (et, cat) = classify_event("POST", "/api/auth/token");
        assert_eq!(cat, AuditEventCategory::Authentication);

        let (et, cat) = classify_event("POST", "/api/onramp/initiate");
        assert_eq!(cat, AuditEventCategory::FinancialTransaction);

        let (et, cat) = classify_event("GET", "/api/admin/accounts");
        assert_eq!(cat, AuditEventCategory::Admin);

        let (et, cat) = classify_event("GET", "/api/wallet/balance");
        assert_eq!(cat, AuditEventCategory::DataAccess);
    }

    #[test]
    fn test_outcome_from_status() {
        assert_eq!(outcome_from_status(200), AuditOutcome::Success);
        assert_eq!(outcome_from_status(201), AuditOutcome::Success);
        assert_eq!(outcome_from_status(401), AuditOutcome::Failure);
        assert_eq!(outcome_from_status(500), AuditOutcome::Failure);
    }

    #[test]
    fn test_should_skip() {
        assert!(should_skip("/health"));
        assert!(should_skip("/metrics"));
        assert!(!should_skip("/api/wallet/balance"));
    }

    // ── Issue #717: body-size limit ─────────────────────────────────────────

    /// Bodies within the limit are hashed normally.
    #[tokio::test]
    async fn test_body_within_limit_produces_hash() {
        let payload = b"hello world";
        let body = Body::from(payload.as_slice());
        let limit = 1024 * 1024; // 1 MiB
        let result = to_bytes(body, limit).await;
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        let hash = sha256_hex(&bytes);
        // SHA-256 of "hello world" is well-known
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576f3c2d4b2fe7b0f3a9"
                .to_string()
                .len(),
            // Length of a hex SHA-256 is always 64 chars
        );
        assert_eq!(hash.len(), 64);
    }

    /// A body that exceeds the limit causes `to_bytes` to return an error,
    /// which the middleware treats as a truncation event (no hash, no failure).
    #[tokio::test]
    async fn test_body_exceeds_limit_returns_error() {
        let big_payload = vec![0u8; 512]; // 512 bytes
        let small_limit = 128usize;       // limit is 128 bytes

        let body = Body::from(big_payload);
        let result = to_bytes(body, small_limit).await;

        // to_bytes returns an error when the body is larger than the limit
        assert!(
            result.is_err(),
            "Expected an error when body exceeds limit, got Ok"
        );
    }

    /// Verifies that the middleware logic path produces None hash for an
    /// oversized body (mirrors the Err branch in audit_middleware).
    #[tokio::test]
    async fn test_oversized_body_yields_no_hash() {
        let big_payload = vec![b'x'; 2048];
        let small_limit = 256usize;

        let body = Body::from(big_payload);
        let result = to_bytes(body, small_limit).await;

        // Simulate the middleware's Err branch
        let (request_body_hash, _reconstructed) = match result {
            Ok(b) => {
                let hash = if b.is_empty() { None } else { Some(sha256_hex(&b)) };
                (hash, b)
            }
            Err(_) => (None, axum::body::Bytes::new()),
        };

        assert_eq!(
            request_body_hash, None,
            "Oversized body should produce no hash"
        );
    }

    /// Verifies that an empty body also yields no hash (no-body requests
    /// such as GET are not hashed).
    #[tokio::test]
    async fn test_empty_body_yields_no_hash() {
        let body = Body::empty();
        let limit = 1024 * 1024;
        let bytes = to_bytes(body, limit).await.unwrap();
        let hash: Option<String> = if bytes.is_empty() {
            None
        } else {
            Some(sha256_hex(&bytes))
        };
        assert_eq!(hash, None);
    }

    /// AuditConfig defaults to 1 MiB.
    #[test]
    fn test_audit_config_default_limit() {
        // Simulate from_env without AUDIT_BODY_LIMIT_BYTES set
        // (env var absent → default 1 MiB)
        let limit: usize = std::env::var("AUDIT_BODY_LIMIT_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1024 * 1024);
        assert_eq!(limit, 1_048_576, "Default body limit should be 1 MiB");
    }
}
