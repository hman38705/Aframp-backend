//! Webhook replay prevention for payment providers (Issue #723)
//!
//! Prevents the same payment webhook event from being processed more than once
//! by storing the event ID in Redis with a TTL that covers the provider's
//! replay window (default: 5 minutes).
//!
//! # How it works
//! 1. Extract the event ID from the parsed [`WebhookEvent`].
//! 2. Attempt an atomic `SET NX EX ttl` on the Redis key
//!    `webhook:replay:<provider>:<event_id>`.
//! 3. If the key already exists (SET NX returned `nil`) the event is a replay
//!    — return `Err(WebhookReplayError::Duplicate)` and increment the
//!    `aframp_webhook_replay_rejected_total{provider}` Prometheus counter.
//! 4. If the key was freshly set the event is novel — return `Ok(())`.
//!
//! # Usage
//! ```rust,ignore
//! let guard = WebhookReplayGuard::new(redis_pool, WebhookReplayConfig::default());
//!
//! match guard.check_and_store(&event).await {
//!     Ok(()) => { /* process the event */ }
//!     Err(WebhookReplayError::Duplicate { event_id, provider }) => {
//!         // Already processed — return HTTP 200 without re-processing
//!         return Ok(StatusCode::OK);
//!     }
//!     Err(e) => { /* Redis error — log and decide whether to proceed */ }
//! }
//! ```
//!
//! # Prometheus metric
//! `aframp_webhook_replay_rejected_total{provider}` — incremented each time a
//! duplicate webhook is detected.

use crate::payments::types::{ProviderName, WebhookEvent};
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, IntCounterVec};
use redis::AsyncCommands;
use std::sync::Arc;
use tracing::{info, warn};

use crate::cache::RedisPool;

// ---------------------------------------------------------------------------
// Prometheus metric
// ---------------------------------------------------------------------------

lazy_static! {
    /// Counter incremented for every detected webhook replay.
    static ref WEBHOOK_REPLAY_REJECTED_TOTAL: IntCounterVec =
        register_int_counter_vec!(
            "aframp_webhook_replay_rejected_total",
            "Total number of duplicate payment webhooks rejected by replay prevention",
            &["provider"]
        )
        .expect("Failed to register aframp_webhook_replay_rejected_total metric");
}

/// Increment the replay-rejection counter for a given provider.
fn inc_replay_counter(provider: &str) {
    WEBHOOK_REPLAY_REJECTED_TOTAL
        .with_label_values(&[provider])
        .inc();
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Replay-prevention configuration for webhook events.
#[derive(Debug, Clone)]
pub struct WebhookReplayConfig {
    /// How long (seconds) to keep a seen event ID in Redis.
    /// Should be at least as long as the provider's replay window.
    /// Default: 300 (5 minutes).
    pub event_ttl_secs: u64,
}

impl Default for WebhookReplayConfig {
    fn default() -> Self {
        Self { event_ttl_secs: 300 }
    }
}

impl WebhookReplayConfig {
    /// Load from the `WEBHOOK_REPLAY_TTL_SECS` environment variable, defaulting to 300.
    pub fn from_env() -> Self {
        let event_ttl_secs = std::env::var("WEBHOOK_REPLAY_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        Self { event_ttl_secs }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by [`WebhookReplayGuard::check_and_store`].
#[derive(Debug, thiserror::Error)]
pub enum WebhookReplayError {
    /// The event has already been seen — a duplicate / replay.
    #[error("duplicate webhook event `{event_id}` from provider `{provider}`")]
    Duplicate {
        event_id: String,
        provider: String,
    },
    /// The event has no identifier — cannot perform dedup.
    #[error("webhook event from provider `{provider}` has no deduplication ID")]
    MissingEventId { provider: String },
    /// Redis error during the SET NX operation.
    #[error("Redis error during webhook replay check: {0}")]
    RedisError(String),
}

// ---------------------------------------------------------------------------
// Redis key helper
// ---------------------------------------------------------------------------

/// Build a Redis key for a webhook event.
///
/// Format: `webhook:replay:<provider>:<event_id>`
pub fn webhook_replay_key(provider: &str, event_id: &str) -> String {
    format!("webhook:replay:{}:{}", provider, event_id)
}

// ---------------------------------------------------------------------------
// Extract a stable event ID from a WebhookEvent
// ---------------------------------------------------------------------------

/// Derive a stable, unique identifier for a webhook event.
///
/// Providers use different field names; this function tries the most common
/// ones in order:
/// 1. `payload["id"]`
/// 2. `payload["data"]["id"]`
/// 3. `payload["data"]["reference"]`
/// 4. `provider_reference` on the struct itself
/// 5. Composite fallback: `<event_type>:<provider_reference>`
pub fn extract_event_id(event: &WebhookEvent) -> Option<String> {
    // Try top-level "id"
    if let Some(id) = event.payload.get("id").and_then(|v| v.as_str()) {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    // Try data.id
    if let Some(id) = event
        .payload
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    // Try data.reference (Paystack)
    if let Some(id) = event
        .payload
        .get("data")
        .and_then(|d| d.get("reference"))
        .and_then(|v| v.as_str())
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    // Try data.TransactionID (M-Pesa)
    if let Some(id) = event
        .payload
        .get("Body")
        .and_then(|b| b.get("stkCallback"))
        .and_then(|s| s.get("CheckoutRequestID"))
        .and_then(|v| v.as_str())
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    // Fallback: use provider_reference if available
    if let Some(ref pref) = event.provider_reference {
        if !pref.is_empty() {
            return Some(format!("{}:{}", event.event_type, pref));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// WebhookReplayGuard
// ---------------------------------------------------------------------------

/// Guards against duplicate webhook delivery using Redis atomic SET NX.
#[derive(Clone)]
pub struct WebhookReplayGuard {
    redis: Arc<RedisPool>,
    config: Arc<WebhookReplayConfig>,
}

impl WebhookReplayGuard {
    pub fn new(redis: Arc<RedisPool>, config: WebhookReplayConfig) -> Self {
        Self {
            redis,
            config: Arc::new(config),
        }
    }

    pub fn from_env(redis: Arc<RedisPool>) -> Self {
        Self::new(redis, WebhookReplayConfig::from_env())
    }

    /// Check whether the event has been seen before.  If not, atomically
    /// record it so subsequent deliveries are rejected.
    ///
    /// Returns:
    /// - `Ok(())` — event is novel; caller should process it.
    /// - `Err(WebhookReplayError::Duplicate)` — event is a replay; caller
    ///   should return HTTP 200 without re-processing.
    /// - `Err(WebhookReplayError::MissingEventId)` — no dedup ID available.
    /// - `Err(WebhookReplayError::RedisError)` — Redis unavailable; caller
    ///   decides whether to fail open or closed.
    pub async fn check_and_store(
        &self,
        event: &WebhookEvent,
    ) -> Result<(), WebhookReplayError> {
        let provider = provider_name_str(&event.provider);

        let event_id = extract_event_id(event).ok_or_else(|| {
            WebhookReplayError::MissingEventId {
                provider: provider.to_string(),
            }
        })?;

        let key = webhook_replay_key(provider, &event_id);
        let ttl = self.config.event_ttl_secs;

        let mut conn = self
            .redis
            .get()
            .await
            .map_err(|e| WebhookReplayError::RedisError(format!("Connection error: {e}")))?;

        // Atomic SET NX EX ttl — returns "OK" if set, nil if already exists.
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(ttl)
            .query_async(&mut *conn)
            .await
            .map_err(|e| WebhookReplayError::RedisError(format!("SET NX error: {e}")))?;

        if result.is_some() {
            // Key was freshly set — event is novel.
            info!(
                provider = %provider,
                event_id = %event_id,
                ttl_secs = ttl,
                "Webhook event accepted (novel)"
            );
            Ok(())
        } else {
            // Key already existed — duplicate delivery.
            warn!(
                provider = %provider,
                event_id = %event_id,
                "Webhook replay detected — ignoring duplicate event"
            );
            inc_replay_counter(provider);
            Err(WebhookReplayError::Duplicate {
                event_id,
                provider: provider.to_string(),
            })
        }
    }

    /// Check without recording (useful for testing / dry-run inspection).
    pub async fn is_duplicate(&self, event: &WebhookEvent) -> bool {
        let provider = provider_name_str(&event.provider);
        let Some(event_id) = extract_event_id(event) else {
            return false;
        };
        let key = webhook_replay_key(provider, &event_id);
        let mut conn = match self.redis.get().await {
            Ok(c) => c,
            Err(_) => return false,
        };
        let exists: bool = conn.exists(&key).await.unwrap_or(false);
        exists
    }
}
}

// ---------------------------------------------------------------------------
// Provider name helper
// ---------------------------------------------------------------------------

fn provider_name_str(provider: &ProviderName) -> &'static str {
    provider.as_str()
}

// ---------------------------------------------------------------------------
// Unit tests (no Redis required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(provider: ProviderName, payload: serde_json::Value) -> WebhookEvent {
        WebhookEvent {
            provider,
            event_type: "charge.success".to_string(),
            transaction_reference: None,
            provider_reference: None,
            status: None,
            payload,
            received_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    // ── extract_event_id ─────────────────────────────────────────────────────

    #[test]
    fn extracts_top_level_id() {
        let event = make_event(ProviderName::Paystack, json!({"id": "evt_123", "event": "charge.success"}));
        assert_eq!(extract_event_id(&event), Some("evt_123".to_string()));
    }

    #[test]
    fn extracts_data_id() {
        let event = make_event(ProviderName::Flutterwave, json!({"data": {"id": "flw_456"}}));
        assert_eq!(extract_event_id(&event), Some("flw_456".to_string()));
    }

    #[test]
    fn extracts_data_reference_for_paystack() {
        let event = make_event(
            ProviderName::Paystack,
            json!({"data": {"reference": "txref_789", "status": "success"}}),
        );
        assert_eq!(extract_event_id(&event), Some("txref_789".to_string()));
    }

    #[test]
    fn extracts_mpesa_checkout_request_id() {
        let event = make_event(
            ProviderName::Mpesa,
            json!({
                "Body": {
                    "stkCallback": {
                        "CheckoutRequestID": "ws_CO_123456",
                        "ResultCode": 0
                    }
                }
            }),
        );
        assert_eq!(extract_event_id(&event), Some("ws_CO_123456".to_string()));
    }

    #[test]
    fn falls_back_to_provider_reference() {
        let mut event = make_event(ProviderName::Paystack, json!({}));
        event.provider_reference = Some("pref_999".to_string());
        assert_eq!(
            extract_event_id(&event),
            Some("charge.success:pref_999".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_id_available() {
        let event = make_event(ProviderName::Paystack, json!({"event": "charge.success"}));
        assert_eq!(extract_event_id(&event), None);
    }

    // ── webhook_replay_key ───────────────────────────────────────────────────

    #[test]
    fn key_format_is_namespaced() {
        let key = webhook_replay_key("paystack", "evt_123");
        assert_eq!(key, "webhook:replay:paystack:evt_123");
    }

    #[test]
    fn keys_differ_across_providers() {
        let k1 = webhook_replay_key("paystack", "evt_1");
        let k2 = webhook_replay_key("flutterwave", "evt_1");
        assert_ne!(k1, k2);
    }

    // ── config ───────────────────────────────────────────────────────────────

    #[test]
    fn default_ttl_is_300() {
        let cfg = WebhookReplayConfig::default();
        assert_eq!(cfg.event_ttl_secs, 300);
    }

    #[test]
    fn config_from_env_reads_override() {
        std::env::set_var("WEBHOOK_REPLAY_TTL_SECS", "600");
        let cfg = WebhookReplayConfig::from_env();
        assert_eq!(cfg.event_ttl_secs, 600);
        std::env::remove_var("WEBHOOK_REPLAY_TTL_SECS");
    }

    // ── provider_name_str ────────────────────────────────────────────────────

    #[test]
    fn provider_names_are_lowercase() {
        assert_eq!(provider_name_str(&ProviderName::Paystack), "paystack");
        assert_eq!(provider_name_str(&ProviderName::Flutterwave), "flutterwave");
        assert_eq!(provider_name_str(&ProviderName::Mpesa), "mpesa");
    }
}
