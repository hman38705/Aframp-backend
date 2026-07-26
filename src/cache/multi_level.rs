//! Multi-level cache manager.
//!
//! Promotion rules:
//! - L1 (moka, in-process): fee structures, currency configs, provider lists.
//!   Low volatility, high read frequency, process-local.
//! - L2 (Redis, distributed): exchange rates, wallet balances, quotes, history cursors.
//!   Shared across instances, moderate volatility.
//!
//! On a cache miss at L1, the manager checks L2 before hitting the database.
//! On a miss at both levels, a single-flight rebuild is triggered so only one
//! request rebuilds the entry while concurrent requests wait.
//!
//! Probabilistic early expiry (via moka's time_to_idle) prevents simultaneous
//! expiry spikes across multiple instances.

use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::Arc;
use tracing::{debug, info};

use crate::cache::cache::{Cache as CacheTrait, RedisCache};
use crate::cache::l1::{L1Cache, L1Category};
use crate::cache::metrics::{CacheSizeMetrics, L1Metrics, L2Metrics};
use crate::cache::single_flight::SingleFlight;

/// The unified multi-level cache handle. Clone-cheap (all fields are Arc).
#[derive(Clone)]
pub struct MultiLevelCache {
    pub l1: L1Cache,
    pub l2: RedisCache,
    pub l1_metrics: Arc<L1Metrics>,
    pub l2_metrics: Arc<L2Metrics>,
    pub size_metrics: Arc<CacheSizeMetrics>,
    /// Per-key single-flight guard (keyed by cache key string).
    sf: Arc<SingleFlight<Vec<u8>>>,
}

impl MultiLevelCache {
    pub fn new(
        l1: L1Cache,
        l2: RedisCache,
        l1_metrics: Arc<L1Metrics>,
        l2_metrics: Arc<L2Metrics>,
        size_metrics: Arc<CacheSizeMetrics>,
    ) -> Self {
        Self {
            l1,
            l2,
            l1_metrics,
            l2_metrics,
            size_metrics,
            sf: SingleFlight::new(),
        }
    }

    // -------------------------------------------------------------------------
    // L1-only helpers (fee structures, currency configs, provider lists)
    // -------------------------------------------------------------------------

    /// Get from L1 only (for low-volatility, process-local data).
    pub async fn l1_get<T: DeserializeOwned>(&self, category: L1Category, key: &str) -> Option<T> {
        let shard = self.l1_shard(category);
        shard.get(key).await
    }

    /// Insert into L1 only.
    pub async fn l1_insert<T: Serialize>(&self, category: L1Category, key: String, value: &T) {
        let shard = self.l1_shard(category);
        shard.insert(key, value).await;
    }

    /// Invalidate a key from L1 only.
    pub async fn l1_invalidate(&self, category: L1Category, key: &str) {
        let shard = self.l1_shard(category);
        shard.invalidate(key).await;
    }

    /// Invalidate all entries in an L1 category.
    pub async fn l1_invalidate_all(&self, category: L1Category) {
        let shard = self.l1_shard(category);
        shard.invalidate_all().await;
    }

    // -------------------------------------------------------------------------
    // L2-only helpers (exchange rates, wallet balances, quotes)
    // -------------------------------------------------------------------------

    /// Get from L2 (Redis) only.
    pub async fn l2_get<T: Serialize + DeserializeOwned + Send + Sync + 'static>(
        &self,
        category: &str,
        key: &str,
    ) -> Option<T> {
        match CacheTrait::<T>::get(&self.l2, key).await {
            Ok(Some(v)) => {
                self.l2_metrics.record_hit(category);
                debug!(category, key, "L2 cache hit");
                Some(v)
            }
            Ok(None) => {
                self.l2_metrics.record_miss(category);
                debug!(category, key, "L2 cache miss");
                None
            }
            Err(e) => {
                debug!(category, key, error = %e, "L2 cache error (degraded)");
                None
            }
        }
    }

    /// Set in L2 (Redis) only.
    pub async fn l2_set<T: Serialize + DeserializeOwned + Send + Sync + 'static>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<std::time::Duration>,
    ) {
        if let Err(e) = self.l2.set(key, value, ttl).await {
            debug!(key, error = %e, "L2 cache set error (degraded)");
        }
    }

    /// Delete from L2 (Redis) only.
    pub async fn l2_invalidate<T>(&self, key: &str)
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        if let Err(e) = CacheTrait::<T>::delete(&self.l2, key).await {
            debug!(key, error = %e, "L2 cache delete error (degraded)");
        } else {
            info!(key, "L2 cache invalidated");
        }
    }

    /// Delete all L2 keys matching a pattern.
    pub async fn l2_invalidate_pattern<T>(&self, pattern: &str)
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        match CacheTrait::<T>::delete_pattern(&self.l2, pattern).await {
            Ok(n) => info!(pattern, deleted = n, "L2 cache pattern invalidated"),
            Err(e) => debug!(pattern, error = %e, "L2 pattern delete error (degraded)"),
        }
    }

    // -------------------------------------------------------------------------
    // Combined invalidation (admin-triggered config updates)
    // -------------------------------------------------------------------------

    /// Invalidate a key from both L1 and L2 simultaneously.
    pub async fn invalidate_both(&self, category: L1Category, l1_key: &str, l2_key: &str) {
        tokio::join!(
            self.l1_invalidate(category, l1_key),
            self.l2_invalidate::<serde_json::Value>(l2_key),
        );
        info!(l1_key, l2_key, "Both cache levels invalidated");
    }

    /// Invalidate every L1 (across all shards) and L2 key whose key starts
    /// with `key_prefix`. Returns the total number of keys removed.
    ///
    /// Intended for admin-triggered invalidation when fee structures,
    /// corridor configs, or provider settings are mutated — since the
    /// mutating handler may not know which cache level(s) or exact key
    /// currently hold the stale value, this covers all three L1 shards plus
    /// an L2 `SCAN`-based pattern delete.
    pub async fn invalidate_prefix(&self, key_prefix: &str) -> u64 {
        let mut removed: u64 = 0;

        for shard in [
            &self.l1.fee_structures,
            &self.l1.currency_configs,
            &self.l1.provider_lists,
        ] {
            for key in shard.keys_with_prefix(key_prefix) {
                shard.invalidate(&key).await;
                removed += 1;
            }
        }

        let l2_pattern = format!("{}*", key_prefix);
        match CacheTrait::<serde_json::Value>::delete_pattern(&self.l2, &l2_pattern).await {
            Ok(n) => removed += n,
            Err(e) => debug!(pattern = %l2_pattern, error = %e, "L2 prefix invalidation failed (degraded)"),
        }

        info!(key_prefix, removed, "Prefix cache invalidation complete");
        removed
    }

    /// [`invalidate_prefix`] plus an audit row in `cache_invalidation_logs`
    /// and the `aframp_cache_invalidation_total{reason}` metric — the entry
    /// point admin mutation handlers should call.
    ///
    /// [`invalidate_prefix`]: MultiLevelCache::invalidate_prefix
    pub async fn invalidate_prefix_logged(
        &self,
        pool: &sqlx::PgPool,
        key_prefix: &str,
        initiator_id: Option<uuid::Uuid>,
        initiator_role: &str,
        reason: &str,
    ) -> u64 {
        let removed = self.invalidate_prefix(key_prefix).await;

        let pattern = format!("{}*", key_prefix);
        if let Err(e) = sqlx::query(
            r#"INSERT INTO cache_invalidation_logs
               (initiator_id, initiator_role, target_namespace, pattern_used, keys_deleted, reason)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(initiator_id)
        .bind(initiator_role)
        .bind(key_prefix)
        .bind(&pattern)
        .bind(removed as i64)
        .bind(reason)
        .execute(pool)
        .await
        {
            debug!(error = %e, "Failed to write cache_invalidation_logs row (degraded)");
        }

        crate::metrics::cache::cache_invalidation_total()
            .with_label_values(&[reason])
            .inc();

        removed
    }

    // -------------------------------------------------------------------------
    // Single-flight get-or-rebuild (stampede protection)
    // -------------------------------------------------------------------------

    /// Get from L2 with single-flight rebuild on miss.
    ///
    /// `rebuild` is called at most once per key regardless of concurrent callers.
    /// All concurrent waiters receive the same rebuilt value.
    pub async fn l2_get_or_rebuild<T, F, Fut>(
        &self,
        category: &str,
        key: &str,
        ttl: std::time::Duration,
        rebuild: F,
    ) -> Result<T, String>
    where
        T: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        // Fast path: L2 hit
        if let Some(v) = self.l2_get::<T>(category, key).await {
            return Ok(v);
        }

        // Slow path: single-flight rebuild
        let l2 = self.l2.clone();
        let key_owned = key.to_string();
        let category_owned = category.to_string();
        let l2_metrics = self.l2_metrics.clone();

        let result_bytes = self
            .sf
            .get_or_rebuild(key, || async move {
                let value = rebuild().await?;
                let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                // Populate L2 after rebuild
                if let Err(e) = l2.set(&key_owned, &value, Some(ttl)).await {
                    debug!(key = key_owned, error = %e, "Failed to populate L2 after rebuild");
                }
                l2_metrics.record_miss(&category_owned);
                Ok(bytes)
            })
            .await?;

        serde_json::from_slice(&result_bytes).map_err(|e| e.to_string())
    }

    // -------------------------------------------------------------------------
    // Size metric updates (call periodically or after warming)
    // -------------------------------------------------------------------------

    pub fn update_size_metrics(&self) {
        self.size_metrics
            .set_l1_size("fee_structures", self.l1.fee_structures.entry_count());
        self.size_metrics
            .set_l1_size("currency_configs", self.l1.currency_configs.entry_count());
        self.size_metrics
            .set_l1_size("provider_lists", self.l1.provider_lists.entry_count());
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn l1_shard(&self, category: L1Category) -> &crate::cache::l1::L1Shard {
        match category {
            L1Category::FeeStructures => &self.l1.fee_structures,
            L1Category::CurrencyConfigs => &self.l1.currency_configs,
            L1Category::ProviderLists => &self.l1.provider_lists,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::l1::L1Cache;
    use prometheus::Registry;

    async fn test_cache() -> MultiLevelCache {
        let registry = Registry::new();
        let l1_metrics = crate::cache::metrics::L1Metrics::new(&registry);
        let l2_metrics = crate::cache::metrics::L2Metrics::new(&registry);
        let size_metrics = crate::cache::metrics::CacheSizeMetrics::new(&registry);
        let l1 = L1Cache::new(l1_metrics.clone());

        // Deliberately unreachable — L2 operations degrade gracefully to no-ops,
        // which is fine since this test only exercises L1 prefix matching.
        let manager = bb8_redis::RedisConnectionManager::new("redis://127.0.0.1:1").unwrap();
        let pool = bb8::Pool::builder()
            .max_size(1)
            .connection_timeout(std::time::Duration::from_millis(50))
            .build_unchecked(manager);
        let redis = RedisCache::new(pool);

        MultiLevelCache::new(l1, redis, l1_metrics.clone(), l2_metrics, size_metrics)
    }

    #[tokio::test]
    async fn test_invalidate_prefix_removes_matching_l1_keys_only() {
        let cache = test_cache().await;

        cache
            .l1_insert(L1Category::FeeStructures, "corridor:ng-us:v1".to_string(), &1)
            .await;
        cache
            .l1_insert(L1Category::FeeStructures, "corridor:ng-us:v2".to_string(), &2)
            .await;
        cache
            .l1_insert(L1Category::FeeStructures, "corridor:gh-ng:v1".to_string(), &3)
            .await;
        cache
            .l1_insert(L1Category::CurrencyConfigs, "corridor:ng-us:currency".to_string(), &4)
            .await;
        cache
            .l1_insert(L1Category::FeeStructures, "unrelated:key".to_string(), &5)
            .await;

        let removed = cache.invalidate_prefix("corridor:ng-us").await;
        assert_eq!(removed, 3, "should remove the 3 keys matching the prefix across shards");

        assert_eq!(
            cache
                .l1_get::<i32>(L1Category::FeeStructures, "corridor:ng-us:v1")
                .await,
            None
        );
        assert_eq!(
            cache
                .l1_get::<i32>(L1Category::FeeStructures, "corridor:ng-us:v2")
                .await,
            None
        );
        assert_eq!(
            cache
                .l1_get::<i32>(L1Category::CurrencyConfigs, "corridor:ng-us:currency")
                .await,
            None
        );

        // Non-matching keys survive.
        assert_eq!(
            cache
                .l1_get::<i32>(L1Category::FeeStructures, "corridor:gh-ng:v1")
                .await,
            Some(3)
        );
        assert_eq!(
            cache.l1_get::<i32>(L1Category::FeeStructures, "unrelated:key").await,
            Some(5)
        );
    }

    #[tokio::test]
    async fn test_invalidate_prefix_no_match_removes_nothing() {
        let cache = test_cache().await;
        cache
            .l1_insert(L1Category::ProviderLists, "provider:a".to_string(), &1)
            .await;

        let removed = cache.invalidate_prefix("nonexistent:").await;
        assert_eq!(removed, 0);
        assert_eq!(
            cache.l1_get::<i32>(L1Category::ProviderLists, "provider:a").await,
            Some(1)
        );
    }
}
