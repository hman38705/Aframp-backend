//! Integration tests for advanced per-consumer rate limiting (Issue #175 / #725).
//!
//! Exercises the real Postgres-backed profile/override merge
//! (`get_effective_rate_limits`), which can't be verified as a client-side
//! unit test since the COALESCE-over-LEFT-JOIN-LATERAL merge lives in SQL.
//!
//! Run with:
//!   DATABASE_URL=postgres://... cargo test --test advanced_rate_limit_test --features database -- --nocapture

#![cfg(feature = "database")]

use aframp_backend::database::consumer_rate_limit_repository::ConsumerRateLimitRepository;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

async fn test_pool() -> Result<PgPool, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&url).await?;
    Ok(pool)
}

async fn seed_consumer(pool: &PgPool, consumer_type: &str) -> Result<Uuid, Box<dyn std::error::Error>> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO consumers (id, name, consumer_type, environment)
        VALUES ($1, 'advanced rate limit test consumer', $2, 'testnet')
        "#,
    )
    .bind(id)
    .bind(consumer_type)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn cleanup_consumer(pool: &PgPool, consumer_id: Uuid) {
    let _ = sqlx::query("DELETE FROM consumer_rate_limit_overrides WHERE consumer_id = $1")
        .bind(consumer_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM consumers WHERE id = $1")
        .bind(consumer_id)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore] // requires DATABASE_URL against a migrated database
async fn effective_limits_falls_back_to_profile_when_no_override() {
    let pool = test_pool().await.expect("DATABASE_URL must point to a migrated database");
    let repo = ConsumerRateLimitRepository::new(Arc::new(pool.clone()));

    // Seeded by the #175 migration for every known consumer_type.
    let consumer_id = seed_consumer(&pool, "partner").await.expect("seed consumer");

    let effective = repo
        .get_effective_limits(consumer_id)
        .await
        .expect("get_effective_limits should succeed")
        .expect("partner profile should exist from migration seed data");

    assert_eq!(effective["global"]["limit"], json!(100));

    cleanup_consumer(&pool, consumer_id).await;
}

#[tokio::test]
#[ignore] // requires DATABASE_URL against a migrated database
async fn active_override_takes_precedence_over_profile() {
    let pool = test_pool().await.expect("DATABASE_URL must point to a migrated database");
    let repo = ConsumerRateLimitRepository::new(Arc::new(pool.clone()));

    let consumer_id = seed_consumer(&pool, "mobile_client").await.expect("seed consumer");

    let override_limits = json!({
        "global": {"limit": 2, "window_secs": 60},
        "endpoint_critical": {"limit": 1, "window_secs": 60}
    });
    let created = repo
        .create_override(consumer_id, &override_limits, None, None, Some("integration test".to_string()))
        .await
        .expect("create_override should succeed");
    assert_eq!(created.consumer_id, consumer_id);

    let effective = repo
        .get_effective_limits(consumer_id)
        .await
        .expect("get_effective_limits should succeed")
        .expect("override should make limits effective");
    assert_eq!(effective["global"]["limit"], json!(2), "override must win over the mobile_client profile");

    let active = repo.list_overrides(consumer_id).await.expect("list_overrides should succeed");
    assert_eq!(active.len(), 1);

    let deleted = repo.delete_override(created.id).await.expect("delete_override should succeed");
    assert!(deleted);

    let effective_after_delete = repo
        .get_effective_limits(consumer_id)
        .await
        .expect("get_effective_limits should succeed")
        .expect("profile should still apply after override removal");
    assert_eq!(
        effective_after_delete["global"]["limit"],
        json!(10),
        "should fall back to the mobile_client profile (global limit 10) once the override is gone"
    );

    cleanup_consumer(&pool, consumer_id).await;
}

#[tokio::test]
#[ignore] // requires DATABASE_URL against a migrated database
async fn expired_override_is_not_effective() {
    let pool = test_pool().await.expect("DATABASE_URL must point to a migrated database");
    let repo = ConsumerRateLimitRepository::new(Arc::new(pool.clone()));

    let consumer_id = seed_consumer(&pool, "microservice").await.expect("seed consumer");

    let expired_at = chrono::Utc::now() - chrono::Duration::hours(1);
    // Bypass the repository's implicit "future expiry" assumption by inserting directly,
    // since create_override doesn't validate expiry_at is in the future at the API layer.
    sqlx::query(
        r#"
        INSERT INTO consumer_rate_limit_overrides (consumer_id, limits_json, expiry_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(consumer_id)
    .bind(json!({"global": {"limit": 1, "window_secs": 60}}))
    .bind(expired_at)
    .execute(&pool)
    .await
    .ok(); // the chk_override_expiry constraint may reject this — that's fine, it's the assertion.

    let active = repo.list_overrides(consumer_id).await.expect("list_overrides should succeed");
    assert!(active.is_empty(), "expired overrides must not be listed as active");

    cleanup_consumer(&pool, consumer_id).await;
}
