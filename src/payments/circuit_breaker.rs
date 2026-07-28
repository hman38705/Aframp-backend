//! Payment Provider Circuit Breaker (Issue #778)
//!
//! Per-provider circuit breaker to prevent thread-pool exhaustion when an
//! upstream payment provider (Paystack, Flutterwave, M-Pesa, Ghana) is
//! degraded.
//!
//! # State Machine
//! ```
//! Closed ──(3 failures / 60 s)──► Open ──(30 s)──► HalfOpen ──(success)──► Closed
//!                                                              └──(fail)──► Open
//! ```
//!
//! # Metrics
//! `aframp_payment_provider_circuit_state{provider}` gauge:
//!   0 = Closed, 1 = Open, 2 = HalfOpen

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use tracing::{info, warn};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of failures within the window before tripping the breaker.
const FAILURE_THRESHOLD: u64 = 3;
/// Rolling window over which failures are counted (seconds).
const FAILURE_WINDOW_SECS: u64 = 60;
/// How long the circuit stays open before moving to HalfOpen (seconds).
const OPEN_DURATION_SECS: u64 = 30;

// ── State enum ────────────────────────────────────────────────────────────────

const STATE_CLOSED: u8 = 0;
const STATE_OPEN: u8 = 1;
const STATE_HALF_OPEN: u8 = 2;

// ── CircuitBreaker ────────────────────────────────────────────────────────────

/// Shared, lock-free circuit breaker for a single payment provider.
#[derive(Debug)]
pub struct CircuitBreaker {
    provider: String,
    state: AtomicU8,
    /// Monotonic failure counter.
    failure_count: AtomicU64,
    /// Unix-second timestamp of the first failure in the current window.
    window_start_secs: AtomicU64,
    /// Unix-second timestamp when the circuit was tripped (state → Open).
    opened_at_secs: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(provider: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            provider: provider.into(),
            state: AtomicU8::new(STATE_CLOSED),
            failure_count: AtomicU64::new(0),
            window_start_secs: AtomicU64::new(0),
            opened_at_secs: AtomicU64::new(0),
        })
    }

    /// Returns `true` when the caller may proceed with the request.
    /// Returns `false` when the circuit is Open (caller must return 503).
    pub fn allow_request(&self) -> bool {
        let now = now_secs();
        match self.state.load(Ordering::Acquire) {
            STATE_CLOSED => true,
            STATE_OPEN => {
                let opened_at = self.opened_at_secs.load(Ordering::Relaxed);
                if now.saturating_sub(opened_at) >= OPEN_DURATION_SECS {
                    // Transition to HalfOpen — let one probe through.
                    self.state
                        .compare_exchange(STATE_OPEN, STATE_HALF_OPEN, Ordering::AcqRel, Ordering::Relaxed)
                        .ok();
                    info!(provider = %self.provider, "Circuit breaker → HalfOpen (probe request allowed)");
                    true
                } else {
                    false
                }
            }
            STATE_HALF_OPEN => {
                // Only one probe at a time; subsequent callers are blocked.
                false
            }
            _ => true,
        }
    }

    /// Record a successful call — closes the circuit if it was HalfOpen.
    pub fn record_success(&self) {
        let prev = self.state.swap(STATE_CLOSED, Ordering::AcqRel);
        if prev != STATE_CLOSED {
            info!(provider = %self.provider, "Circuit breaker → Closed after successful probe");
            self.failure_count.store(0, Ordering::Relaxed);
        }
        emit_metric(&self.provider, STATE_CLOSED);
    }

    /// Record a failed call.  Trips the breaker when the threshold is hit.
    pub fn record_failure(&self) {
        let now = now_secs();
        let window_start = self.window_start_secs.load(Ordering::Relaxed);

        // Reset window if it has expired.
        if now.saturating_sub(window_start) >= FAILURE_WINDOW_SECS {
            self.window_start_secs.store(now, Ordering::Relaxed);
            self.failure_count.store(1, Ordering::Relaxed);
            emit_metric(&self.provider, self.state.load(Ordering::Relaxed));
            return;
        }

        let count = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;

        if count >= FAILURE_THRESHOLD {
            let prev_state = self.state.swap(STATE_OPEN, Ordering::AcqRel);
            if prev_state != STATE_OPEN {
                self.opened_at_secs.store(now, Ordering::Relaxed);
                warn!(
                    provider = %self.provider,
                    failures = count,
                    window_secs = FAILURE_WINDOW_SECS,
                    "Circuit breaker → Open (failure threshold exceeded)"
                );
                emit_metric(&self.provider, STATE_OPEN);
            }
        }
    }

    /// True when the circuit is Open (requests must be rejected).
    pub fn is_open(&self) -> bool {
        self.state.load(Ordering::Acquire) == STATE_OPEN
    }

    /// Build the 503 response with a `Retry-After` header.
    pub fn open_response(&self) -> Response {
        let opened_at = self.opened_at_secs.load(Ordering::Relaxed);
        let now = now_secs();
        let elapsed = now.saturating_sub(opened_at);
        let retry_after = OPEN_DURATION_SECS.saturating_sub(elapsed).max(1);

        let mut resp = (
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "Payment provider '{}' is temporarily unavailable. Retry after {} seconds.",
                self.provider, retry_after
            ),
        )
            .into_response();

        resp.headers_mut().insert(
            "Retry-After",
            HeaderValue::from_str(&retry_after.to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("30")),
        );
        resp
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Emit Prometheus gauge `aframp_payment_provider_circuit_state{provider}`.
/// Uses plain `tracing` so it works without a full Prometheus registry.
fn emit_metric(provider: &str, state: u8) {
    let state_label = match state {
        STATE_CLOSED => "closed",
        STATE_OPEN => "open",
        STATE_HALF_OPEN => "half_open",
        _ => "unknown",
    };
    tracing::info!(
        metric = "aframp_payment_provider_circuit_state",
        provider = provider,
        state = state_label,
        value = state,
        "circuit breaker state updated"
    );
}

// ── Registry ──────────────────────────────────────────────────────────────────

/// One-stop shop: call `get()` to obtain the circuit breaker for a provider.
/// Breakers are created lazily and held for the lifetime of the process.
pub struct CircuitBreakerRegistry {
    paystack: Arc<CircuitBreaker>,
    flutterwave: Arc<CircuitBreaker>,
    mpesa: Arc<CircuitBreaker>,
    ghana: Arc<CircuitBreaker>,
}

impl CircuitBreakerRegistry {
    pub fn new() -> Self {
        Self {
            paystack: CircuitBreaker::new("paystack"),
            flutterwave: CircuitBreaker::new("flutterwave"),
            mpesa: CircuitBreaker::new("mpesa"),
            ghana: CircuitBreaker::new("ghana"),
        }
    }

    pub fn get(&self, provider: &str) -> Arc<CircuitBreaker> {
        match provider {
            "paystack" => Arc::clone(&self.paystack),
            "flutterwave" => Arc::clone(&self.flutterwave),
            "mpesa" | "mpesa_kenya" => Arc::clone(&self.mpesa),
            "ghana" => Arc::clone(&self.ghana),
            _ => CircuitBreaker::new(provider), // fallback: unshared
        }
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
