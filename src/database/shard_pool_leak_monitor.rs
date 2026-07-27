//! Connection-leak monitoring for per-shard pools (`DatabaseShardConfig`).
//!
//! `sqlx::PgPool` exposes live `size()` / `num_idle()` counters. A leaked
//! connection (checked out and never returned) shows up as `size() ==
//! max_connections` while `num_idle()` stays low even under no load. Today
//! nothing watches for that pattern per-shard, so a leak is only noticed once
//! the pool is fully exhausted and requests start timing out.
//!
//! This module adds an opt-in poller that periodically samples each shard's
//! pool and warns when a shard looks saturated, so it can be caught early.

use std::time::Duration;

use sqlx::PgPool;
use tracing::warn;

/// Fraction of `max_connections` in use, sustained with little idle capacity,
/// that is treated as a possible leak rather than normal load.
const SATURATION_RATIO: f64 = 0.9;

/// One sample of a shard's pool utilization.
#[derive(Debug, Clone, Copy)]
pub struct ShardPoolSample {
    pub shard_id: i16,
    pub size: u32,
    pub idle: u32,
    pub max_connections: u32,
}

impl ShardPoolSample {
    pub fn in_use(&self) -> u32 {
        self.size.saturating_sub(self.idle)
    }

    /// True when the pool is saturated enough that a leak is plausible.
    pub fn looks_leaked(&self) -> bool {
        if self.max_connections == 0 {
            return false;
        }
        (self.in_use() as f64 / self.max_connections as f64) >= SATURATION_RATIO
    }
}

pub fn sample(shard_id: i16, pool: &PgPool, max_connections: u32) -> ShardPoolSample {
    ShardPoolSample {
        shard_id,
        size: pool.size(),
        idle: pool.num_idle() as u32,
        max_connections,
    }
}

/// Spawn a background task that periodically samples every shard pool in
/// `pools` and logs a warning for any shard that looks saturated/leaked.
pub fn spawn_leak_watcher(pools: Vec<(i16, PgPool, u32)>, poll_interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(poll_interval);
        loop {
            ticker.tick().await;
            for (shard_id, pool, max_connections) in &pools {
                let s = sample(*shard_id, pool, *max_connections);
                if s.looks_leaked() {
                    warn!(
                        shard_id = s.shard_id,
                        in_use = s.in_use(),
                        max_connections = s.max_connections,
                        "shard pool near exhaustion — possible connection leak"
                    );
                }
            }
        }
    });
}
