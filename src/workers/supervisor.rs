//! Worker Supervisor (Issue #781)
//!
//! Wraps `tokio::spawn` with panic catching, structured logging, Prometheus
//! metrics, and exponential-backoff restart for all background workers.
//!
//! # Usage
//! ```rust,ignore
//! let supervisor = WorkerSupervisor::new();
//! supervisor.spawn("stellar_confirmation", move || {
//!     let worker = worker.clone();
//!     let rx = shutdown_rx.clone();
//!     async move { worker.run(rx).await }
//! });
//! ```
//!
//! # Metrics
//! `aframp_worker_panic_total{worker_name}` — incremented on each panic.
//!
//! # Health
//! `WorkerSupervisor::health_report()` returns per-worker heartbeat state
//! suitable for the `/health` endpoint.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tracing::{error, info, warn};

// ── Backoff constants ─────────────────────────────────────────────────────────

const BACKOFF_INITIAL_MS: u64 = 1_000;
const BACKOFF_MAX_MS: u64 = 60_000;

// ── Per-worker health record ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorkerHealth {
    pub name: String,
    /// Total panics since startup.
    pub panic_count: u64,
    /// When the worker last sent a heartbeat (spawned / restarted).
    pub last_heartbeat: Option<Instant>,
    /// Whether the worker is currently believed to be running.
    pub running: bool,
}

// ── Supervisor ────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct WorkerSupervisor {
    health: Arc<Mutex<HashMap<String, WorkerHealth>>>,
}

impl WorkerSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a supervised worker.
    ///
    /// `factory` is a closure that, when called, returns a `Future` that drives
    /// the worker.  On panic the closure is called again after the backoff delay.
    pub fn spawn<F, Fut>(&self, name: &'static str, factory: F) -> JoinHandle<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let health = Arc::clone(&self.health);

        // Initialise health record.
        {
            let mut map = health.lock().unwrap();
            map.insert(
                name.to_string(),
                WorkerHealth {
                    name: name.to_string(),
                    panic_count: 0,
                    last_heartbeat: None,
                    running: false,
                },
            );
        }

        tokio::spawn(async move {
            let mut backoff_ms = BACKOFF_INITIAL_MS;

            loop {
                // Mark as running and record heartbeat.
                {
                    let mut map = health.lock().unwrap();
                    if let Some(h) = map.get_mut(name) {
                        h.running = true;
                        h.last_heartbeat = Some(Instant::now());
                    }
                }

                // Drive the worker inside `catch_unwind`-equivalent via
                // `AssertUnwindSafe` so we can detect panics without aborting.
                let fut = std::panic::AssertUnwindSafe(factory());
                let result = futures::FutureExt::catch_unwind(fut).await;

                match result {
                    Ok(()) => {
                        // Worker returned normally (e.g. shutdown signal).
                        info!(worker = name, "Worker exited cleanly — not restarting");
                        let mut map = health.lock().unwrap();
                        if let Some(h) = map.get_mut(name) {
                            h.running = false;
                        }
                        break;
                    }
                    Err(panic_payload) => {
                        // Extract a human-readable message from the panic payload.
                        let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                            (*s).to_string()
                        } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "<non-string panic payload>".to_string()
                        };

                        error!(
                            worker = name,
                            panic_message = %msg,
                            backoff_ms,
                            "Worker panicked — will restart after backoff"
                        );

                        // Increment panic counter and emit metric.
                        let new_count = {
                            let mut map = health.lock().unwrap();
                            if let Some(h) = map.get_mut(name) {
                                h.panic_count += 1;
                                h.running = false;
                                h.panic_count
                            } else {
                                1
                            }
                        };

                        emit_panic_metric(name, new_count);

                        // Exponential backoff — capped at BACKOFF_MAX_MS.
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms = (backoff_ms * 2).min(BACKOFF_MAX_MS);
                    }
                }
            }
        })
    }

    /// Return a snapshot of all worker health records.
    pub fn health_report(&self) -> Vec<WorkerHealth> {
        self.health
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }
}

// ── Metric emission ───────────────────────────────────────────────────────────

fn emit_panic_metric(worker_name: &str, total: u64) {
    tracing::warn!(
        metric = "aframp_worker_panic_total",
        worker_name = worker_name,
        value = total,
        "worker panic metric"
    );
}
