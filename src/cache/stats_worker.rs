//! Cache stats background worker (Issue #459)
//!
//! Polls every 60 seconds:
//! - Redis INFO memory → updates gauges, alerts if ≥ 90% of maxmemory
//! - MultiLevelCache L1 entry counts → updates size gauges

use crate::cache::cache::RedisCache;
use crate::cache::multi_level::MultiLevelCache;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

const POLL_INTERVAL_SECS: u64 = 60;
const MEMORY_ALERT_PCT: f64 = 90.0;
const PENDING_ALERT_THRESHOLD: i64 = 10;

pub struct CacheStatsWorker {
    cache: Arc<MultiLevelCache>,
    redis: Arc<RedisCache>,
}

impl CacheStatsWorker {
    pub fn new(cache: Arc<MultiLevelCache>, redis: Arc<RedisCache>) -> Self {
        Self { cache, redis }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
            loop {
                ticker.tick().await;
                self.run_cycle().await;
            }
        });
    }

    async fn run_cycle(&self) {
        // Update L1 size gauges
        self.cache.update_size_metrics();

        // Fetch and check Redis memory
        self.check_redis_memory().await;

        // Update bb8 pool gauges and check for exhaustion
        self.check_pool_stats();
    }

    fn check_pool_stats(&self) {
        let state = self.redis.pool.state();
        let pending = self.redis.pending_acquisitions();

        crate::metrics::cache::redis_pool_size()
            .with_label_values(&["primary"])
            .set(state.connections as f64);
        crate::metrics::cache::redis_pool_idle()
            .with_label_values(&["primary"])
            .set(state.idle_connections as f64);
        crate::metrics::cache::redis_pool_pending()
            .with_label_values(&["primary"])
            .set(pending as f64);

        if pending > PENDING_ALERT_THRESHOLD {
            warn!(
                pending,
                threshold = PENDING_ALERT_THRESHOLD,
                pool_size = state.connections,
                idle = state.idle_connections,
                "ALERT: Redis pool has {} connections pending — pool may be exhausted", pending
            );
        }
    }

    async fn check_redis_memory(&self) {
        let mut conn = match self.redis.get_connection().await {
            Ok(c) => c,
            Err(_) => return,
        };

        let info: String = match redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut *conn)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "CacheStatsWorker: Redis INFO memory failed");
                return;
            }
        };

        let used = parse_field(&info, "used_memory:").unwrap_or(0) as f64;
        let maxmem = parse_field(&info, "maxmemory:").unwrap_or(0) as f64;

        // Update Prometheus gauges (aframp_redis_memory_used_bytes, aframp_redis_maxmemory_bytes)
        crate::metrics::cache::redis_memory_used_bytes()
            .with_label_values(&["primary"])
            .set(used);
        crate::metrics::cache::redis_maxmemory_bytes()
            .with_label_values(&["primary"])
            .set(maxmem);

        if maxmem > 0.0 {
            let pct = used / maxmem * 100.0;
            if pct >= MEMORY_ALERT_PCT {
                warn!(
                    used_mb = used / 1_048_576.0,
                    maxmemory_mb = maxmem / 1_048_576.0,
                    usage_pct = pct,
                    "ALERT: Redis memory usage ≥{}% of maxmemory", MEMORY_ALERT_PCT
                );
            } else {
                info!(
                    usage_pct = pct,
                    "CacheStatsWorker: Redis memory OK"
                );
            }
        }
    }
}

fn parse_field(info: &str, field: &str) -> Option<u64> {
    info.lines()
        .find(|l| l.starts_with(field))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<u64>().ok())
}
