//! Integration tests for payment webhook replay prevention (Issue #723).
//!
//! Requires a running Redis instance.
//! Run with: cargo test --features cache replay_prevention_test -- --ignored

use std::sync::Arc;

use aframp_backend::payments::types::{ProviderName, WebhookEvent};
use aframp_backend::payments::webhook_replay::{
    extract_event_id, webhook_replay_key, WebhookReplayConfig, WebhookReplayError,
    WebhookReplayGuard,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_event(
    provider: ProviderName,
    event_id: &str,
    event_type: &str,
) -> WebhookEvent {
    WebhookEvent {
        provider,
        event_type: event_type.to_string(),
        transaction_reference: None,
        provider_reference: Some(event_id.to_string()),
        status: None,
        payload: serde_json::json!({
            "id": event_id,
            "event": event_type
        }),
        received_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn paystack_event(event_id: &str) -> WebhookEvent {
    WebhookEvent {
        provider: ProviderName::Paystack,
        event_type: "charge.success".to_string(),
        transaction_reference: None,
        provider_reference: Some(event_id.to_string()),
        status: None,
        payload: serde_json::json!({
            "event": "charge.success",
            "data": { "reference": event_id, "status": "success" }
        }),
        received_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn flutterwave_event(event_id: &str) -> WebhookEvent {
    WebhookEvent {
        provider: ProviderName::Flutterwave,
        event_type: "charge.completed".to_string(),
        transaction_reference: None,
        provider_reference: None,
        status: None,
        payload: serde_json::json!({
            "event": "charge.completed",
            "data": { "id": event_id, "status": "successful" }
        }),
        received_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn mpesa_event(checkout_id: &str) -> WebhookEvent {
    WebhookEvent {
        provider: ProviderName::Mpesa,
        event_type: "stk.callback".to_string(),
        transaction_reference: None,
        provider_reference: None,
        status: None,
        payload: serde_json::json!({
            "Body": {
                "stkCallback": {
                    "CheckoutRequestID": checkout_id,
                    "ResultCode": 0
                }
            }
        }),
        received_at: chrono::Utc::now().to_rfc3339(),
    }
}

async fn build_guard() -> WebhookReplayGuard {
    use aframp_backend::cache::{init_cache_pool, CacheConfig};
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .expect("Redis init failed");
    WebhookReplayGuard::new(
        Arc::new(pool),
        WebhookReplayConfig { event_ttl_secs: 10 }, // short TTL for tests
    )
}

fn unique_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!(
        "test_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// A novel Paystack event must be accepted.
#[tokio::test]
#[ignore]
async fn paystack_novel_event_accepted() {
    let guard = build_guard().await;
    let event = paystack_event(&unique_id());
    assert!(guard.check_and_store(&event).await.is_ok());
}

/// A replayed Paystack event must be rejected with Duplicate error.
#[tokio::test]
#[ignore]
async fn paystack_duplicate_event_rejected() {
    let guard = build_guard().await;
    let id = unique_id();
    let event = paystack_event(&id);

    // First delivery — accepted
    guard.check_and_store(&event).await.expect("first should succeed");

    // Second delivery (replay) — rejected
    let result = guard.check_and_store(&event).await;
    match result {
        Err(WebhookReplayError::Duplicate { event_id, provider }) => {
            assert!(event_id.contains(&id) || event_id.contains("charge.success"));
            assert_eq!(provider, "paystack");
        }
        other => panic!("expected Duplicate, got {:?}", other),
    }
}

/// A novel Flutterwave event must be accepted.
#[tokio::test]
#[ignore]
async fn flutterwave_novel_event_accepted() {
    let guard = build_guard().await;
    let event = flutterwave_event(&unique_id());
    assert!(guard.check_and_store(&event).await.is_ok());
}

/// A replayed Flutterwave event must be rejected.
#[tokio::test]
#[ignore]
async fn flutterwave_duplicate_event_rejected() {
    let guard = build_guard().await;
    let id = unique_id();
    let event = flutterwave_event(&id);

    guard.check_and_store(&event).await.expect("first should succeed");
    let result = guard.check_and_store(&event).await;
    assert!(
        matches!(result, Err(WebhookReplayError::Duplicate { .. })),
        "expected Duplicate, got {:?}", result
    );
}

/// A novel M-Pesa STK callback must be accepted.
#[tokio::test]
#[ignore]
async fn mpesa_novel_event_accepted() {
    let guard = build_guard().await;
    let event = mpesa_event(&unique_id());
    assert!(guard.check_and_store(&event).await.is_ok());
}

/// A replayed M-Pesa event must be rejected.
#[tokio::test]
#[ignore]
async fn mpesa_duplicate_event_rejected() {
    let guard = build_guard().await;
    let checkout_id = unique_id();
    let event = mpesa_event(&checkout_id);

    guard.check_and_store(&event).await.expect("first should succeed");
    let result = guard.check_and_store(&event).await;
    assert!(
        matches!(result, Err(WebhookReplayError::Duplicate { .. })),
        "expected Duplicate, got {:?}", result
    );
}

/// Two concurrent requests with the same event ID — only one must succeed.
/// Verifies the atomic SET NX prevents race conditions.
#[tokio::test]
#[ignore]
async fn concurrent_duplicate_events_only_one_accepted() {
    let guard = build_guard().await;
    let id = unique_id();
    let e1 = paystack_event(&id);
    let e2 = paystack_event(&id);

    let guard2 = guard.clone();
    let (r1, r2) = tokio::join!(guard.check_and_store(&e1), guard2.check_and_store(&e2));

    let ok_count = [r1.is_ok(), r2.is_ok()].iter().filter(|&&x| x).count();
    let dup_count = [
        matches!(r1.as_ref().err(), Some(WebhookReplayError::Duplicate { .. })),
        matches!(r2.as_ref().err(), Some(WebhookReplayError::Duplicate { .. })),
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    assert_eq!(ok_count, 1, "exactly one concurrent delivery should succeed");
    assert_eq!(dup_count, 1, "exactly one should be rejected as duplicate");
}

/// Same event ID from different providers must NOT interfere.
#[tokio::test]
#[ignore]
async fn same_id_different_providers_both_accepted() {
    let guard = build_guard().await;
    let shared_id = unique_id();

    let paystack = paystack_event(&shared_id);
    let flutterwave = flutterwave_event(&shared_id);

    // Both should succeed because they're namespaced by provider
    assert!(
        guard.check_and_store(&paystack).await.is_ok(),
        "paystack should be accepted"
    );
    assert!(
        guard.check_and_store(&flutterwave).await.is_ok(),
        "flutterwave should be accepted with same event id"
    );
}

/// `is_duplicate` returns false for fresh events and true after storing.
#[tokio::test]
#[ignore]
async fn is_duplicate_reflects_stored_state() {
    let guard = build_guard().await;
    let id = unique_id();
    let event = paystack_event(&id);

    // Before store
    assert!(!guard.is_duplicate(&event).await);

    // Store it
    guard.check_and_store(&event).await.unwrap();

    // After store
    assert!(guard.is_duplicate(&event).await);
}

// ---------------------------------------------------------------------------
// Unit tests (no Redis)
// ---------------------------------------------------------------------------

#[test]
fn extract_event_id_paystack_data_reference() {
    let event = WebhookEvent {
        provider: ProviderName::Paystack,
        event_type: "charge.success".to_string(),
        transaction_reference: None,
        provider_reference: None,
        status: None,
        payload: serde_json::json!({"data": {"reference": "txref_abc"}}),
        received_at: chrono::Utc::now().to_rfc3339(),
    };
    assert_eq!(extract_event_id(&event), Some("txref_abc".to_string()));
}

#[test]
fn extract_event_id_flutterwave_data_id() {
    let event = WebhookEvent {
        provider: ProviderName::Flutterwave,
        event_type: "charge.completed".to_string(),
        transaction_reference: None,
        provider_reference: None,
        status: None,
        payload: serde_json::json!({"data": {"id": "flw_12345"}}),
        received_at: chrono::Utc::now().to_rfc3339(),
    };
    assert_eq!(extract_event_id(&event), Some("flw_12345".to_string()));
}

#[test]
fn extract_event_id_top_level_id_wins() {
    let event = WebhookEvent {
        provider: ProviderName::Paystack,
        event_type: "charge.success".to_string(),
        transaction_reference: None,
        provider_reference: None,
        status: None,
        payload: serde_json::json!({
            "id": "top_level_id",
            "data": {"reference": "data_ref"}
        }),
        received_at: chrono::Utc::now().to_rfc3339(),
    };
    // top-level "id" takes priority
    assert_eq!(extract_event_id(&event), Some("top_level_id".to_string()));
}

#[test]
fn webhook_replay_key_namespaced_by_provider() {
    let k_ps = webhook_replay_key("paystack", "evt_001");
    let k_flw = webhook_replay_key("flutterwave", "evt_001");
    assert_ne!(k_ps, k_flw);
    assert_eq!(k_ps, "webhook:replay:paystack:evt_001");
    assert_eq!(k_flw, "webhook:replay:flutterwave:evt_001");
}
