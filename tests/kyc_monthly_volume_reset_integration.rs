//! Integration tests for atomic month-boundary reset of KYC monthly volume
//! tracking (src/database/kyc_repository.rs, src/kyc/limits.rs).
//!
//! These exercise real PostgreSQL row-locking behavior, so they can't be
//! written as no-DB unit tests: the guarantee under test ("a stale monthly
//! counter can't be double-counted or lost when a reset races a concurrent
//! transaction") only exists because of how the UPSERT and reset UPDATE are
//! phrased against the database, not any client-side logic.
//!
//! Run with:
//!   DATABASE_URL=postgres://... cargo test --test kyc_monthly_volume_reset_integration --features database -- --nocapture
//!
//! Each test creates its own consumer and cleans up after itself.

#![cfg(feature = "database")]

use aframp_backend::database::kyc_repository::KycRepository;
use chrono::Datelike;
use sqlx::types::BigDecimal;
use sqlx::{PgPool, Row};
use std::str::FromStr;
use uuid::Uuid;

async fn test_pool() -> Result<PgPool, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&url).await?;
    Ok(pool)
}

async fn seed_consumer(pool: &PgPool) -> Result<Uuid, Box<dyn std::error::Error>> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO consumers (id, name, consumer_type, environment)
        VALUES ($1, 'kyc monthly volume test consumer', 'backend_microservice', 'testnet')
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn cleanup(pool: &PgPool, consumer_id: Uuid) {
    let _ = sqlx::query("DELETE FROM kyc_monthly_volume_trackers WHERE consumer_id = $1")
        .bind(consumer_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM consumers WHERE id = $1")
        .bind(consumer_id)
        .execute(pool)
        .await;
}

/// Seed a stale monthly tracker row (as if last written N months ago) so
/// tests can exercise the rollover path deterministically instead of
/// waiting for a real month boundary.
async fn seed_stale_row(
    pool: &PgPool,
    consumer_id: Uuid,
    months_ago: i32,
    monthly_volume: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        r#"
        INSERT INTO kyc_monthly_volume_trackers (consumer_id, month_start, monthly_volume, last_reset_at, updated_at)
        VALUES (
            $1,
            date_trunc('month', now() - ($2 || ' months')::interval)::date,
            $3,
            now() - ($2 || ' months')::interval,
            now() - ($2 || ' months')::interval
        )
        "#,
    )
    .bind(consumer_id)
    .bind(months_ago.to_string())
    .bind(BigDecimal::from_str(monthly_volume)?)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_upserts_across_a_stale_month_roll_over_without_double_counting() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = test_pool().await?;
    let consumer_id = seed_consumer(&pool).await?;

    // Simulate a counter last touched two months ago with a large leftover
    // balance that must NOT survive into this month's total.
    seed_stale_row(&pool, consumer_id, 2, "999999.00").await?;

    let repo = KycRepository::new(pool.clone());

    const CONCURRENT_WRITERS: usize = 20;
    const AMOUNT_PER_WRITER: &str = "10.00";

    let mut handles = Vec::with_capacity(CONCURRENT_WRITERS);
    for _ in 0..CONCURRENT_WRITERS {
        let repo_pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let repo = KycRepository::new(repo_pool);
            repo.upsert_monthly_volume(consumer_id, BigDecimal::from_str(AMOUNT_PER_WRITER).unwrap())
                .await
        }));
    }

    for handle in handles {
        handle.await??;
    }

    let row = sqlx::query("SELECT monthly_volume, month_start FROM kyc_monthly_volume_trackers WHERE consumer_id = $1")
        .bind(consumer_id)
        .fetch_one(&pool)
        .await?;

    let final_volume: BigDecimal = row.try_get("monthly_volume")?;
    let month_start: chrono::NaiveDate = row.try_get("month_start")?;

    let expected = BigDecimal::from_str(AMOUNT_PER_WRITER)? * BigDecimal::from(CONCURRENT_WRITERS as i64);
    assert_eq!(
        final_volume, expected,
        "monthly volume must equal exactly the sum of concurrent writes after rollover (no double-count, no lost update)"
    );
    assert_eq!(
        month_start,
        chrono::Utc::now().date_naive().with_day(1).unwrap(),
        "stale month_start must have rolled over to the current month"
    );

    // Sanity check via the read path used by limits.rs.
    let read_back = repo.get_monthly_volume_used(consumer_id).await?;
    assert_eq!(read_back, expected);

    cleanup(&pool, consumer_id).await;
    Ok(())
}

#[tokio::test]
async fn reset_stale_monthly_counters_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let consumer_id = seed_consumer(&pool).await?;

    seed_stale_row(&pool, consumer_id, 1, "500.00").await?;

    let repo = KycRepository::new(pool.clone());

    let first_run = repo.reset_stale_monthly_counters().await?;
    assert_eq!(first_run, 1, "first run should reset the one stale row");

    let row = sqlx::query("SELECT monthly_volume, month_start FROM kyc_monthly_volume_trackers WHERE consumer_id = $1")
        .bind(consumer_id)
        .fetch_one(&pool)
        .await?;
    let volume: BigDecimal = row.try_get("monthly_volume")?;
    let month_start: chrono::NaiveDate = row.try_get("month_start")?;
    assert_eq!(volume, BigDecimal::from(0));
    assert_eq!(month_start, chrono::Utc::now().date_naive().with_day(1).unwrap());

    let second_run = repo.reset_stale_monthly_counters().await?;
    assert_eq!(second_run, 0, "re-running in the same month must be a no-op");

    cleanup(&pool, consumer_id).await;
    Ok(())
}

#[tokio::test]
async fn fresh_consumer_reads_zero_monthly_volume() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let consumer_id = seed_consumer(&pool).await?;

    let repo = KycRepository::new(pool.clone());
    let volume = repo.get_monthly_volume_used(consumer_id).await?;
    assert_eq!(volume, BigDecimal::from(0));

    cleanup(&pool, consumer_id).await;
    Ok(())
}
