//! Single-flight pattern for cache stampede protection.
//!
//! When multiple concurrent requests miss the cache for the same key,
//! only one rebuild is triggered. All other waiters receive the same result
//! once the rebuild completes, preventing a thundering-herd against the DB.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info};

type SharedResult<T> = Arc<Result<T, String>>;

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
    /// Returns `Ok(value)` on success, `Err(msg)` if the rebuild failed.
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
                match rx.recv().await {
                    Ok(result) => (*result).clone().map_err(|e| e.clone()),
                    Err(_) => Err(format!(
                        "single-flight leader for {key} ended without producing a result"
                    )),
                }
            }
            Role::Leader(tx) => {
                info!(key, "single-flight: leader rebuilding cache entry");
                let result = rebuild().await;
                let shared: SharedResult<T> = Arc::new(result.clone().map_err(|e| e.to_string()));

                // Broadcast result to all waiters (ignore send errors — no subscribers is fine).
                let _ = tx.send(shared);

                // Remove from in-flight map.
                let mut map = self.in_flight.lock().await;
                map.remove(key);

                result
            }
        }
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
}
