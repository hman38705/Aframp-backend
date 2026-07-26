/// Dynamic fee adjustment engine for Stellar surge pricing
///
/// Queries Horizon's /fee_stats endpoint to determine network congestion levels
/// and dynamically adjusts transaction submission fees to guarantee inclusion
/// within the immediate next ledger.
///
/// Concurrent-refresh protection: on cache expiry, many requests can arrive at
/// once (e.g. after a burst of submissions). Without a guard, all of them would
/// hit Horizon simultaneously — a stampede. `get_fee_stats` uses the shared
/// `SingleFlight` guard so only one caller per instance actually calls Horizon;
/// the rest await that single in-flight fetch. TTLs are jittered per-entry so
/// multiple instances (and repeated refreshes on the same instance) don't all
/// expire in lock-step. If Horizon is unreachable, the last-known-good fee
/// estimate is served instead of failing the caller.
use crate::cache::single_flight::SingleFlight;
use crate::stellar::error::{SubmissionError, SubmissionResult};
use crate::stellar::models::{FeeConfiguration, FeeStats};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// TTL jitter as a fraction of the base TTL (±20%), applied on every cache
/// write so entries don't all expire at the same instant.
const TTL_JITTER_FRACTION: f64 = 0.2;

const SINGLE_FLIGHT_KEY: &str = "fee_stats";

/// Dynamic fee engine with caching and surge pricing
pub struct DynamicFeeEngine {
    config: FeeConfiguration,
    cache: Arc<RwLock<FeeCache>>,
    horizon_client: reqwest::Client,
    horizon_url: String,
    /// Guards concurrent Horizon `/fee_stats` fetches against a stampede.
    single_flight: Arc<SingleFlight<FeeStats>>,
}

struct FeeCache {
    last_stats: Option<CachedFeeStats>,
    base_ttl_seconds: i64,
}

struct CachedFeeStats {
    stats: FeeStats,
    cached_at: DateTime<Utc>,
    /// Jittered TTL for this entry specifically (computed at write time).
    ttl_seconds: i64,
}

/// Apply ±`TTL_JITTER_FRACTION` jitter to a base TTL.
fn jittered_ttl_seconds(base_seconds: i64) -> i64 {
    let factor = rand::thread_rng()
        .gen_range((1.0 - TTL_JITTER_FRACTION)..=(1.0 + TTL_JITTER_FRACTION));
    ((base_seconds as f64) * factor).round().max(1.0) as i64
}

impl DynamicFeeEngine {
    /// Create a new dynamic fee engine
    pub fn new(config: FeeConfiguration, horizon_url: String) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(FeeCache {
                last_stats: None,
                base_ttl_seconds: 10,
            })),
            horizon_client: reqwest::Client::new(),
            horizon_url,
            single_flight: SingleFlight::new(),
        }
    }

    /// Query current fee stats from Horizon with caching.
    ///
    /// Cache expiry triggers at most one concurrent Horizon fetch per instance
    /// (via `SingleFlight`); if that fetch fails, the last-known-good stats are
    /// returned instead of propagating the error, provided one exists.
    pub async fn get_fee_stats(&self) -> SubmissionResult<FeeStats> {
        if let Some(stats) = self.fresh_cached_stats().await {
            return Ok(stats);
        }

        let horizon_client = self.horizon_client.clone();
        let url = format!("{}/fee_stats", self.horizon_url);

        let fetch_result = self
            .single_flight
            .get_or_rebuild(SINGLE_FLIGHT_KEY, || async move {
                let response = horizon_client
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                    .map_err(|e| format!("fee_stats request failed: {}", e))?;

                let stats: FeeStats = response
                    .json()
                    .await
                    .map_err(|e| format!("failed to parse fee_stats response: {}", e))?;

                Ok(stats)
            })
            .await;

        match fetch_result {
            Ok(stats) => {
                self.store_cached_stats(stats.clone()).await;
                Ok(stats)
            }
            Err(err) => {
                if let Some(stale) = self.last_known_good().await {
                    warn!(
                        error = %err,
                        "Horizon fee_stats fetch failed; falling back to last-known-good fee estimate"
                    );
                    Ok(stale)
                } else {
                    Err(SubmissionError::HorizonApi(err))
                }
            }
        }
    }

    /// Cached stats if present and within their (jittered) TTL.
    async fn fresh_cached_stats(&self) -> Option<FeeStats> {
        let cache = self.cache.read().await;
        let entry = cache.last_stats.as_ref()?;
        let age = Utc::now().signed_duration_since(entry.cached_at);
        (age.num_seconds() < entry.ttl_seconds).then(|| entry.stats.clone())
    }

    /// Last cached stats regardless of freshness — used as the Horizon-unreachable fallback.
    async fn last_known_good(&self) -> Option<FeeStats> {
        let cache = self.cache.read().await;
        cache.last_stats.as_ref().map(|e| e.stats.clone())
    }

    async fn store_cached_stats(&self, stats: FeeStats) {
        let mut cache = self.cache.write().await;
        let ttl_seconds = jittered_ttl_seconds(cache.base_ttl_seconds);
        cache.last_stats = Some(CachedFeeStats {
            stats,
            cached_at: Utc::now(),
            ttl_seconds,
        });
    }

    /// Calculate optimal submission fee based on current network conditions.
    ///
    /// Uses accepted fee percentiles to reduce overpayment under normal conditions,
    /// while still escalating during surge windows.
    pub async fn calculate_fee(&self, operation_count: i32) -> SubmissionResult<i64> {
        let fee_stats = self.get_fee_stats().await?;
        let base_fee = fee_stats.last_ledger_base_fee.max(self.config.min_fee);

        // Check network capacity
        let capacity_usage: f64 = fee_stats.network_capacity_usage.parse().unwrap_or(0.5);

        let percentile_fee = if capacity_usage > self.config.surge_threshold {
            fee_stats.percentile_90_accepted_fee
        } else {
            fee_stats.percentile_50_accepted_fee
        }
        .max(base_fee);

        let per_op_fee = if capacity_usage > self.config.surge_threshold {
            ((percentile_fee as f64) * self.config.surge_multiplier) as i64
        } else {
            percentile_fee
        }
        .clamp(self.config.min_fee, self.config.max_fee);

        // max_fee is interpreted as max fee per operation.
        let ops = operation_count.max(1) as i64;
        Ok(per_op_fee * ops)
    }

    /// Calculate fee with explicit surge multiplier (for testing)
    pub fn calculate_fee_with_multiplier(
        &self,
        operation_count: i32,
        base_fee: i64,
        surge_multiplier: f64,
    ) -> i64 {
        let per_op_fee = (base_fee as f64 * surge_multiplier) as i64;
        let per_op_fee = per_op_fee.min(self.config.max_fee).max(self.config.min_fee);
        let total_fee = per_op_fee * operation_count as i64;
        total_fee.min(self.config.max_fee).max(self.config.min_fee)
    }

    /// Get the surge fee percentage (100 = no surge, 150 = 50% surge)
    pub async fn get_surge_percent(&self) -> SubmissionResult<Decimal> {
        let fee_stats = self.get_fee_stats().await?;
        let capacity_usage: f64 = fee_stats.network_capacity_usage.parse().unwrap_or(0.5);

        let multiplier = if capacity_usage > self.config.surge_threshold {
            self.config.surge_multiplier
        } else {
            1.0
        };

        let percent = (multiplier * 100.0) as i64;
        Ok(sqlx::types::Decimal::from(percent))
    }

    /// Estimate dynamic-fee savings vs using static configured base fee.
    /// Returns percent saved (0-100).
    pub async fn estimate_savings_percent(&self, operation_count: i32) -> SubmissionResult<f64> {
        let dynamic = self.calculate_fee(operation_count).await? as f64;
        let static_fee =
            (self.config.base_fee.max(self.config.min_fee) * operation_count.max(1) as i64) as f64;

        if static_fee <= 0.0 {
            return Ok(0.0);
        }

        let savings = ((static_fee - dynamic) / static_fee) * 100.0;
        Ok(savings.max(0.0))
    }

    /// Get current capacity usage percentage
    pub async fn get_capacity_usage(&self) -> SubmissionResult<f64> {
        let fee_stats = self.get_fee_stats().await?;
        fee_stats
            .network_capacity_usage
            .parse()
            .map_err(|_| SubmissionError::FeeCalculationError("invalid capacity usage".to_string()))
    }

    /// Clear the fee cache (for testing)
    #[cfg(test)]
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.last_stats = None;
    }

    /// Force the current cache entry (if any) to read as expired, without
    /// waiting out its TTL in real time. Test-only.
    #[cfg(test)]
    pub async fn force_expire_for_test(&self) {
        let mut cache = self.cache.write().await;
        if let Some(entry) = cache.last_stats.as_mut() {
            entry.cached_at = Utc::now() - Duration::seconds(entry.ttl_seconds + 1);
        }
    }

    /// Get cached fee stats if available
    pub async fn get_cached_stats(&self) -> SubmissionResult<Option<FeeStats>> {
        Ok(self.last_known_good().await)
    }
}

// Import Decimal type for use in methods
use sqlx::types::Decimal;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_fee_config() -> FeeConfiguration {
        FeeConfiguration {
            base_fee: 100,
            min_fee: 100,
            max_fee: 10_000,
            surge_threshold: 0.8,
            surge_multiplier: 1.5,
            low_capacity_fee: 1_000,
        }
    }

    fn create_test_engine() -> DynamicFeeEngine {
        DynamicFeeEngine::new(
            test_fee_config(),
            "https://horizon-testnet.stellar.org".to_string(),
        )
    }

    fn sample_fee_stats_json(last_ledger: i64) -> serde_json::Value {
        serde_json::json!({
            "ledger_capacity_usage": "0.5",
            "last_ledger_base_fee": 100,
            "last_ledger": last_ledger,
            "network_capacity_usage": "0.5",
            "percentile_10_accepted_fee": 100,
            "percentile_20_accepted_fee": 100,
            "percentile_30_accepted_fee": 100,
            "percentile_40_accepted_fee": 100,
            "percentile_50_accepted_fee": 100,
            "percentile_60_accepted_fee": 100,
            "percentile_70_accepted_fee": 100,
            "percentile_80_accepted_fee": 100,
            "percentile_90_accepted_fee": 150,
            "percentile_95_accepted_fee": 200,
            "percentile_99_accepted_fee": 300
        })
    }

    #[test]
    fn test_calculate_fee_with_multiplier_no_surge() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(1, 100, 1.0);
        assert_eq!(fee, 100);
    }

    #[test]
    fn test_calculate_fee_with_multiplier_surge() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(1, 100, 1.5);
        assert_eq!(fee, 150);
    }

    #[test]
    fn test_calculate_fee_respects_max() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(100, 100, 2.0);
        assert_eq!(fee, 10_000); // Capped at max_fee
    }

    #[test]
    fn test_calculate_fee_respects_min() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(1, 50, 0.5);
        assert_eq!(fee, 100); // Floor at min_fee
    }

    #[test]
    fn test_calculate_fee_multi_operation() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(5, 100, 1.0);
        assert_eq!(fee, 500);
    }

    // ── Horizon stampede protection ─────────────────────────────────────────

    #[tokio::test]
    async fn test_concurrent_cache_miss_hits_horizon_exactly_once() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(150))
                    .set_body_json(sample_fee_stats_json(12345)),
            )
            .mount(&mock_server)
            .await;

        let engine = Arc::new(DynamicFeeEngine::new(test_fee_config(), mock_server.uri()));

        // 20 concurrent callers, all missing a cold cache at once — the
        // scenario that used to stampede Horizon before SingleFlight was wired in.
        let handles: Vec<_> = (0..20)
            .map(|_| {
                let engine = engine.clone();
                tokio::spawn(async move { engine.get_fee_stats().await })
            })
            .collect();

        for h in handles {
            let stats = h.await.unwrap().expect("fee stats fetch should succeed");
            assert_eq!(stats.last_ledger, 12345);
        }

        let received = mock_server.received_requests().await.unwrap();
        assert_eq!(
            received.len(),
            1,
            "expected exactly one Horizon request across 20 concurrent callers"
        );
    }

    #[tokio::test]
    async fn test_fresh_cache_avoids_refetching_horizon() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_fee_stats_json(1)))
            .mount(&mock_server)
            .await;

        let engine = DynamicFeeEngine::new(test_fee_config(), mock_server.uri());
        engine.get_fee_stats().await.unwrap();
        engine.get_fee_stats().await.unwrap();
        engine.get_fee_stats().await.unwrap();

        let received = mock_server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1, "fresh cache hits must not re-fetch Horizon");
    }

    #[tokio::test]
    async fn test_falls_back_to_last_known_good_when_horizon_unreachable() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_responder = call_count.clone();

        // First request succeeds and populates the cache; every request after
        // that fails, deterministically (no reliance on wiremock mock-priority
        // ordering between two separate `Mock`s).
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(move |_: &wiremock::Request| {
                if call_count_responder.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(200).set_body_json(sample_fee_stats_json(999))
                } else {
                    ResponseTemplate::new(503)
                }
            })
            .mount(&mock_server)
            .await;

        let engine = DynamicFeeEngine::new(test_fee_config(), mock_server.uri());

        let first = engine
            .get_fee_stats()
            .await
            .expect("first fetch should succeed and populate the cache");
        assert_eq!(first.last_ledger, 999);

        engine.force_expire_for_test().await;

        // Horizon now fails every request; the engine must serve the
        // last-known-good value instead of propagating the error.
        let second = engine
            .get_fee_stats()
            .await
            .expect("should fall back to last-known-good instead of erroring");
        assert_eq!(second.last_ledger, 999);
    }

    #[tokio::test]
    async fn test_errors_when_horizon_unreachable_and_no_cache_exists() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let engine = DynamicFeeEngine::new(test_fee_config(), mock_server.uri());
        let result = engine.get_fee_stats().await;
        assert!(result.is_err(), "no last-known-good exists, so the error must surface");
    }

    #[test]
    fn test_ttl_jitter_stays_within_bounds() {
        for _ in 0..500 {
            let ttl = jittered_ttl_seconds(10);
            assert!((8..=12).contains(&ttl), "jittered ttl {ttl} outside ±20% of base 10s");
        }
    }

    #[test]
    fn test_ttl_jitter_varies() {
        let samples: std::collections::HashSet<i64> =
            (0..200).map(|_| jittered_ttl_seconds(100)).collect();
        assert!(
            samples.len() > 1,
            "expected jitter to produce varying TTLs across repeated calls"
        );
    }
}
