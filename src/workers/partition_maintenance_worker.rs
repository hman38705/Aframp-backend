//! Partition Maintenance Worker
//!
//! Migration `20270630000000_automated_maintenance_partitioning.sql` defines
//! `create_future_partitions(table, days_ahead)` and only schedules it via
//! `pg_cron` when that extension is installed. Managed Postgres instances
//! without `pg_cron` (or with it disabled) would silently never create new
//! partitions ahead of time, so this worker is an application-side fallback
//! that calls the same SQL function on a daily interval.

use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

/// Tables managed by `create_future_partitions`, see the partitioning migration.
const PARTITIONED_TABLES: &[&str] = &["risk_exposure_snapshots", "partner_performance_logs"];

/// How many days of partitions to keep pre-created.
const DAYS_AHEAD: i32 = 7;

/// Run the partition maintenance loop forever, creating upcoming partitions
/// once per day.
pub async fn run(pool: PgPool) {
    let mut ticker = interval(Duration::from_secs(24 * 60 * 60));
    loop {
        ticker.tick().await;
        for table in PARTITIONED_TABLES {
            match sqlx::query_scalar::<_, i32>("SELECT create_future_partitions($1, $2)")
                .bind(table)
                .bind(DAYS_AHEAD)
                .fetch_one(&pool)
                .await
            {
                Ok(created) => {
                    info!(table = %table, partitions_created = created, "Partition maintenance run complete");
                }
                Err(e) => {
                    error!(table = %table, "Failed to create future partitions: {e}");
                }
            }
        }
    }
}
