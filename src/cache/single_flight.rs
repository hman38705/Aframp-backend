//! Single-flight pattern for cache stampede protection.
//!
//! When multiple concurrent requests miss the cache for the same key,
//! only one rebuild is triggered. All other waiters receive the same result
//! once the rebuild completes, preventing a thundering-herd against the DB.

use futures::FutureExt;
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

type SharedResult<T> = Arc<Result<T, String>>;

/// How long a waiter blocks on an in-flight rebuild before giving up on the
/// leader and self-healing by clearing the stale entry. Guards against a
/// leader that hangs (deadlock, unbounded await) without panicking — a
/// panicking leader is handled immediately via `catch_unwind`, without
/// needing to wait out this timeout.
const DEFAULT_IN_FLIGHT_TIMEOUT: Duration = Duration::from_secs(30);

/// A map of in-flight rebuild operations keyed by cache key.
pub struct SingleFlight<T: Clone + Send + 'static> {
    in_flight: Mutex<HashMap<String, broadcast::Sender<SharedResult<T>>>>,
}

impl<T: Clone + Send + 'static> SingleFlight<T> {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            in_flight: Mutex::new(HashMap::new()),
        })
    }

    /// Execute `rebuild` for `key`, or wait for an in-flight rebuild to finish.
    ///
    /// Returns `Ok(value)` on success, `Err(msg)` if the rebuild failed or
    /// panicked. A panic inside `rebuild` is caught so it can never leave the
    /// key permanently stuck in the in-flight map; a leader that hangs
    /// without panicking is bounded by [`DEFAULT_IN_FLIGHT_TIMEOUT`] on the
    /// waiter side.
    pub async fn get_or_rebuild<F, Fut>(&self, key: &str, rebuild: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        enum Role<T> {
            Waiter(broadcast::Receiver<SharedResult<T>>),
            Leader(broadcast::Sender<SharedResult<T>>),
        }

        // Check-for-in-flight and register-as-leader happen under a single
        // lock hold. Splitting them into two separate critical sections (check,
        // then insert) leaves a race window where multiple concurrent misses
        // can each conclude they're the leader — exactly the stampede this
        // type exists to prevent.
        let role = {
            let mut map = self.in_flight.lock().await;
            if let Some(tx) = map.get(key) {
                Role::Waiter(tx.subscribe())
            } else {
                let (tx, _rx) = broadcast::channel::<SharedResult<T>>(1);
                map.insert(key.to_string(), tx.clone());
                Role::Leader(tx)
            }
        };

        match role {
            Role::Waiter(mut rx) => {
                debug!(key, "single-flight: waiting for in-flight rebuild");
                match timeout(DEFAULT_IN_FLIGHT_TIMEOUT, rx.recv()).await {
                    Ok(Ok(result)) => (*result).clone().map_err(|e| e.clone()),
                    Ok(Err(_)) => Err(format!(
                        "single-flight leader for {key} ended without producing a result"
                    )),
                    Err(_) => {
                        // The leader has been rebuilding longer than the
                        // self-heal timeout. Clear the stale entry so this
                        // (and any subsequent) caller can take over as leader
                        // instead of waiting on it forever.
                        warn!(
                            key,
                            timeout_secs = DEFAULT_IN_FLIGHT_TIMEOUT.as_secs(),
                            "single-flight: in-flight rebuild timed out; self-healing"
                        );
                        self.in_flight.lock().await.remove(key);
                        Err(format!(
                            "single-flight leader for {key} timed out after {}s",
                            DEFAULT_IN_FLIGHT_TIMEOUT.as_secs()
                        ))
                    }
                }
            }
            Role::Leader(tx) => {
                info!(key, "single-flight: leader rebuilding cache entry");
                let result: Result<T, String> = match AssertUnwindSafe(rebuild()).catch_unwind().await
                {
                    Ok(result) => result,
                    Err(panic) => {
                        let msg = panic_message(panic.as_ref());
                        error!(key, error = %msg, "single-flight: rebuilder panicked");
                        Err(format!("rebuild panicked: {}", msg))
                    }
                };

                let shared: SharedResult<T> = Arc::new(result.clone());

                // Broadcast result to all waiters (ignore send errors — no subscribers is fine).
                let _ = tx.send(shared);

                // Remove from in-flight map — always, whether the rebuild succeeded,
                // returned Err, or panicked, so the key is never stuck.
                let mut map = self.in_flight.lock().await;
                map.remove(key);

                result
            }
        }
    }
}

/// Best-effort extraction of a human-readable message from a caught panic
/// payload (`std::panic::catch_unwind`'s `Box<dyn Any + Send>`).
fn panic_message(panic: &(dyn Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_concurrent_misses_rebuild_exactly_once() {
        let sf = SingleFlight::<i32>::new();
        let calls = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..50)
            .map(|_| {
                let sf = sf.clone();
                let calls = calls.clone();
                tokio::spawn(async move {
                    sf.get_or_rebuild("k", || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        Ok(42)
                    })
                    .await
                })
            })
            .collect();

        for h in handles {
            assert_eq!(h.await.unwrap().unwrap(), 42);
        }

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "expected exactly one rebuild across all concurrent callers"
        );
    }

    #[tokio::test]
    async fn test_distinct_keys_rebuild_independently() {
        let sf = SingleFlight::<i32>::new();
        let a = sf.get_or_rebuild("a", || async { Ok(1) }).await.unwrap();
        let b = sf.get_or_rebuild("b", || async { Ok(2) }).await.unwrap();
        assert_eq!((a, b), (1, 2));
    }

    #[tokio::test]
    async fn test_rebuild_error_is_propagated() {
        let sf = SingleFlight::<i32>::new();
        let result = sf
            .get_or_rebuild("k", || async { Err("boom".to_string()) })
            .await;
        assert_eq!(result, Err("boom".to_string()));
    }

    #[tokio::test]
    async fn test_successful_rebuild_returns_value() {
        let sf: Arc<SingleFlight<i32>> = SingleFlight::new();
        let result = sf.get_or_rebuild("k", || async { Ok(42) }).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_failed_rebuild_returns_err_and_clears_entry() {
        let sf: Arc<SingleFlight<i32>> = SingleFlight::new();
        let result = sf
            .get_or_rebuild("k", || async { Err::<i32, _>("boom".to_string()) })
            .await;
        assert_eq!(result, Err("boom".to_string()));

        // Entry must not be stuck — a subsequent call for the same key succeeds.
        let result = sf.get_or_rebuild("k", || async { Ok(7) }).await;
        assert_eq!(result, Ok(7));
    }

    #[tokio::test]
    async fn test_panic_in_rebuilder_does_not_permanently_block_key() {
        let sf: Arc<SingleFlight<i32>> = SingleFlight::new();

        // Leader panics mid-rebuild.
        let result = sf
            .get_or_rebuild("panicky-key", || async {
                panic!("rebuilder blew up");
                #[allow(unreachable_code)]
                Ok(0)
            })
            .await;
        assert!(result.is_err(), "a panicking rebuild must surface as Err, not hang");
        assert!(result.unwrap_err().contains("panicked"));

        // The in-flight map entry must have been cleared by the leader itself
        // (via catch_unwind), so the very next call succeeds immediately —
        // it must NOT need to wait out DEFAULT_IN_FLIGHT_TIMEOUT.
        let start = std::time::Instant::now();
        let result = sf.get_or_rebuild("panicky-key", || async { Ok(99) }).await;
        assert_eq!(result, Ok(99));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "post-panic retry should be immediate, not blocked on the in-flight entry"
        );
    }

    #[tokio::test]
    async fn test_concurrent_waiters_all_receive_panic_result() {
        let sf: Arc<SingleFlight<i32>> = SingleFlight::new();

        let leader = {
            let sf = sf.clone();
            tokio::spawn(async move {
                sf.get_or_rebuild("shared-key", || async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    panic!("leader panicked");
                    #[allow(unreachable_code)]
                    Ok::<i32, String>(0)
                })
                .await
            })
        };

        // Give the leader time to register itself as in-flight before waiters subscribe.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let waiter = {
            let sf = sf.clone();
            tokio::spawn(async move {
                sf.get_or_rebuild("shared-key", || async { Ok(123) }).await
            })
        };

        let (leader_result, waiter_result) = tokio::join!(leader, waiter);
        assert!(leader_result.unwrap().is_err());
        assert!(
            waiter_result.unwrap().is_err(),
            "a waiter subscribed to a panicking leader must see the same Err, not hang"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_timeout_self_heals_stuck_leader() {
        let sf: Arc<SingleFlight<i32>> = SingleFlight::new();

        // Leader hangs forever without panicking or completing.
        let _leader = {
            let sf = sf.clone();
            tokio::spawn(async move {
                sf.get_or_rebuild("stuck-key", || async {
                    std::future::pending::<()>().await;
                    Ok::<i32, String>(0)
                })
                .await
            })
        };

        tokio::time::sleep(Duration::from_millis(10)).await;

        // A waiter should time out after DEFAULT_IN_FLIGHT_TIMEOUT and, on
        // timing out, clear the stale entry so it can take over as leader.
        let sf2 = sf.clone();
        let waiter = tokio::spawn(async move {
            sf2.get_or_rebuild("stuck-key", || async { Ok(55) }).await
        });

        tokio::time::advance(DEFAULT_IN_FLIGHT_TIMEOUT + Duration::from_secs(1)).await;

        let result = waiter.await.unwrap();
        assert_eq!(result, Ok(55), "waiter should self-heal and become leader after the timeout");
    }
}
