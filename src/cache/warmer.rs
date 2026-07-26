//! Cache warming — pre-populates L1 and L2 caches at startup.
//!
//! Runs as a detached background task (see `main.rs`) so it never blocks the
//! server from accepting traffic. While warming is in progress the health
//! endpoint reports a degraded-but-serving `Warming` state with a progress
//! percentage (`WarmingState::progress_pct`, published as the
//! `aframp_cache_warmup_progress_pct` gauge) rather than `Unhealthy` — the
//! previous behavior caused load balancers to restart instances mid-warmup.
//! Warming duration and entry counts are logged as structured events.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

use crate::cache::cache::ttl;
use crate::cache::cache::{Cache as CacheTrait, RedisCache};
use crate::cache::l1::L1Cache;
use crate::database::exchange_rate_repository::ExchangeRateRepository;
use crate::database::fee_structure_repository::FeeStructureRepository;

/// Shared flag indicating whether cache warming has completed, plus a
/// 0-100 progress counter so the health endpoint and `aframp_cache_warmup_progress_pct`
/// metric can report partial progress instead of a binary ready/not-ready.
#[derive(Clone)]
pub struct WarmingState {
    pub ready: Arc<AtomicBool>,
    progress_pct: Arc<AtomicU8>,
}

impl WarmingState {
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            progress_pct: Arc::new(AtomicU8::new(0)),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Current warmup progress, 0-100.
    pub fn progress_pct(&self) -> u8 {
        self.progress_pct.load(Ordering::Acquire)
    }

    /// Update warmup progress (0-100) and publish it to the
    /// `aframp_cache_warmup_progress_pct` gauge.
    fn set_progress(&self, pct: u8) {
        let pct = pct.min(100);
        self.progress_pct.store(pct, Ordering::Release);
        crate::metrics::cache::cache_warmup_progress_pct().set(pct as f64);
    }

    pub fn mark_ready(&self) {
        self.set_progress(100);
        self.ready.store(true, Ordering::Release);
    }
}

/// Known currency pairs to pre-warm in L2.
/// Extend this list as new pairs are added to the platform.
const CURRENCY_PAIRS: &[(&str, &str)] = &[
    ("CNGN", "USD"),
    ("CNGN", "EUR"),
    ("CNGN", "GBP"),
    ("CNGN", "KES"),
    ("CNGN", "GHS"),
    ("CNGN", "ZAR"),
    ("CNGN", "XOF"),
    ("USD", "CNGN"),
];

/// Known fee types to pre-warm in L1.
const FEE_TYPES: &[&str] = &["onramp", "offramp", "transfer", "conversion", "withdrawal"];

/// Warm both cache levels. Called once at startup before traffic is accepted.
pub async fn warm_caches(
    l1: &L1Cache,
    redis: &RedisCache,
    rate_repo: &ExchangeRateRepository,
    fee_repo: &FeeStructureRepository,
    warming_state: &WarmingState,
) {
    let start = Instant::now();
    info!("🔥 Starting cache warming...");

    let mut total_l1 = 0usize;
    let mut total_l2 = 0usize;

    let total_items = FEE_TYPES.len() + CURRENCY_PAIRS.len();
    let mut completed_items = 0usize;
    warming_state.set_progress(0);

    // --- L1: fee structures ---
    for fee_type in FEE_TYPES {
        match fee_repo.get_active_by_type(fee_type, None).await {
            Ok(structures) if !structures.is_empty() => {
                let key = format!("v1:fee:structure:{}", fee_type);
                l1.fee_structures.insert(key, &structures).await;
                total_l1 += 1;
                info!(
                    fee_type,
                    count = structures.len(),
                    "L1 warmed fee structures"
                );
            }
            Ok(_) => {
                debug!(fee_type, "No active fee structures found during warming");
            }
            Err(e) => {
                warn!(fee_type, error = %e, "Failed to warm fee structures for type");
            }
        }
        completed_items += 1;
        warming_state.set_progress(((completed_items * 100) / total_items) as u8);
    }

    // --- L2: exchange rates for all known currency pairs ---
    for (from, to) in CURRENCY_PAIRS {
        let key = format!("v1:rate:{}:{}", from, to);
        match rate_repo.get_current_rate(from, to).await {
            Ok(Some(rate)) => {
                if let Err(e) = redis.set(&key, &rate, Some(ttl::EXCHANGE_RATES)).await {
                    warn!(from, to, error = %e, "Failed to warm L2 exchange rate");
                } else {
                    total_l2 += 1;
                    info!(from, to, "L2 warmed exchange rate");
                }
            }
            Ok(None) => {
                debug!(from, to, "No exchange rate found during warming");
            }
            Err(e) => {
                warn!(from, to, error = %e, "Failed to fetch exchange rate for warming");
            }
        }
        completed_items += 1;
        warming_state.set_progress(((completed_items * 100) / total_items) as u8);
    }

    let elapsed = start.elapsed();
    info!(
        elapsed_ms = elapsed.as_millis(),
        l1_entries = total_l1,
        l2_entries = total_l2,
        "✅ Cache warming complete"
    );

    warming_state.mark_ready();
}

// Allow dead_code for the debug macro used in non-debug builds
#[allow(unused_imports)]
use tracing::debug;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warming_state_starts_not_ready_at_zero_progress() {
        let ws = WarmingState::new();
        assert!(!ws.is_ready());
        assert_eq!(ws.progress_pct(), 0);
    }

    #[test]
    fn test_mark_ready_sets_full_progress() {
        let _ = crate::metrics::registry();
        let ws = WarmingState::new();
        ws.mark_ready();
        assert!(ws.is_ready());
        assert_eq!(ws.progress_pct(), 100);
    }

    #[test]
    fn test_set_progress_clamps_to_100() {
        let _ = crate::metrics::registry();
        let ws = WarmingState::new();
        ws.set_progress(255);
        assert_eq!(ws.progress_pct(), 100);
    }
}
