/// Channel account pooling for parallelized sequence number management
///
/// Maintains a pool of channel accounts, each with independent sequence number
/// tracking. Handles rotation, circuit breaking, and load balancing across channels.
use crate::stellar::error::{SubmissionError, SubmissionResult};
use crate::stellar::models::{ChannelHandle, CircuitBreakerState, SubmissionChannel};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// Default number of callers allowed to wait concurrently for a channel slot
/// once every channel is at its in-flight cap.
pub const DEFAULT_QUEUE_DEPTH: usize = 500;
/// Default time a caller will wait in the queue before giving up.
pub const DEFAULT_QUEUE_TIMEOUT: Duration = Duration::from_secs(10);
/// Backoff between retries while waiting for a channel to free up.
const QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// RAII guard: increments `aframp_stellar_queue_depth` on creation, decrements
/// it on drop — covers every exit path (success, error, or the caller
/// dropping the future early).
struct QueueDepthGuard;

impl QueueDepthGuard {
    fn new() -> Self {
        crate::metrics::stellar::queue_depth().inc();
        Self
    }
}

impl Drop for QueueDepthGuard {
    fn drop(&mut self) {
        crate::metrics::stellar::queue_depth().dec();
    }
}

/// Pool of submission channels with load balancing and circuit breaking
pub struct ChannelPool {
    pool: PgPool,
    issuer_id: Uuid,
    channels: Arc<tokio::sync::RwLock<Vec<ChannelHandle>>>,
    current_index: Arc<std::sync::atomic::AtomicUsize>,
    circuit_breaker_threshold: u32,
    max_in_flight_per_channel: u32,
    /// Bounds how many callers may wait concurrently for a channel slot —
    /// the backpressure queue's "depth".
    queue: Arc<Semaphore>,
    queue_timeout: Duration,
}

impl ChannelPool {
    /// Create a new channel pool for an issuer, using the default queue depth
    /// (500) and queue timeout (10s). Use [`ChannelPool::with_queue_config`]
    /// to configure these explicitly.
    pub async fn new(
        pool: PgPool,
        issuer_id: Uuid,
        circuit_breaker_threshold: u32,
        max_in_flight_per_channel: u32,
    ) -> SubmissionResult<Self> {
        Self::with_queue_config(
            pool,
            issuer_id,
            circuit_breaker_threshold,
            max_in_flight_per_channel,
            DEFAULT_QUEUE_DEPTH,
            DEFAULT_QUEUE_TIMEOUT,
        )
        .await
    }

    /// Create a new channel pool with an explicit backpressure queue depth
    /// and timeout.
    pub async fn with_queue_config(
        pool: PgPool,
        issuer_id: Uuid,
        circuit_breaker_threshold: u32,
        max_in_flight_per_channel: u32,
        queue_depth: usize,
        queue_timeout: Duration,
    ) -> SubmissionResult<Self> {
        let channels = Arc::new(tokio::sync::RwLock::new(Vec::new()));
        let pool_obj = Self {
            pool,
            issuer_id,
            channels,
            current_index: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            circuit_breaker_threshold,
            max_in_flight_per_channel,
            queue: Arc::new(Semaphore::new(queue_depth)),
            queue_timeout,
        };

        pool_obj.reload_from_database().await?;
        Ok(pool_obj)
    }

    /// Load channels from database
    pub async fn reload_from_database(&self) -> SubmissionResult<()> {
        let db_channels: Vec<SubmissionChannel> = sqlx::query_as(
            r#"
            SELECT
                id, issuer_id, environment, channel_account_id, channel_index,
                current_sequence, reserved_sequence, balance_xlm, min_balance_threshold,
                is_active, in_rotation, total_submitted, total_successful, total_failed,
                consecutive_failures, last_error_code, last_error_at, created_at, updated_at
            FROM stellar_submission_channels
            WHERE issuer_id = $1 AND is_active = true
            ORDER BY channel_index ASC
            "#,
        )
        .bind(self.issuer_id)
        .fetch_all(&self.pool)
        .await?;

        let mut handles = Vec::new();
        for ch in db_channels {
            let handle = ChannelHandle {
                db_id: ch.id,
                account_id: ch.channel_account_id,
                index: ch.channel_index,
                sequence_counter: Arc::new(AtomicI64::new(ch.current_sequence)),
                reserved_counter: Arc::new(AtomicI64::new(ch.reserved_sequence)),
                submission_count: Arc::new(AtomicU64::new(ch.total_submitted as u64)),
                success_count: Arc::new(AtomicU64::new(ch.total_successful as u64)),
                failure_count: Arc::new(AtomicU64::new(ch.total_failed as u64)),
                circuit_breaker_state: Arc::new(tokio::sync::Mutex::new(CircuitBreakerState {
                    consecutive_failures: ch.consecutive_failures as u32,
                    threshold: self.circuit_breaker_threshold,
                    is_open: ch.consecutive_failures as u32 >= self.circuit_breaker_threshold,
                    last_failure_at: ch.last_error_at,
                })),
            };
            handles.push(handle);
        }

        let mut channels = self.channels.write().await;
        *channels = handles;

        Ok(())
    }

    /// Select the next available channel using round-robin load balancing
    pub async fn select_channel(&self) -> SubmissionResult<ChannelHandle> {
        let channels = self.channels.read().await;

        if channels.is_empty() {
            return Err(SubmissionError::NoActiveChannels);
        }

        // Find first non-broken channel
        for _ in 0..channels.len() {
            let idx = self
                .current_index
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                % channels.len();

            let channel = &channels[idx];
            let cb_state = channel.circuit_breaker_state.lock().await;

            if !cb_state.is_open {
                drop(cb_state);
                return Ok(channel.clone());
            }
        }

        Err(SubmissionError::NoActiveChannels)
    }

    /// Get a specific channel by index
    pub async fn get_channel(&self, index: i32) -> SubmissionResult<ChannelHandle> {
        let channels = self.channels.read().await;
        channels
            .iter()
            .find(|ch| ch.index == index)
            .cloned()
            .ok_or_else(|| SubmissionError::ChannelRotationError("channel not found".to_string()))
    }

    /// Reserve a sequence number from the next available channel
    pub async fn reserve_sequence(&self) -> SubmissionResult<(ChannelHandle, i64)> {
        let channel = self.select_channel().await?;

        let current = channel.sequence_counter.load(Ordering::SeqCst);
        let reserved_now = channel.reserved_counter.load(Ordering::SeqCst);
        let in_flight = reserved_now - current;

        if in_flight >= self.max_in_flight_per_channel as i64 {
            return Err(SubmissionError::ChannelExhausted(format!(
                "channel {} exhausted: {} in-flight (max {})",
                channel.index, in_flight, self.max_in_flight_per_channel
            )));
        }

        // Atomically reserve next sequence for this channel.
        let reserved = channel
            .reserved_counter
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);

        // Persist reserved sequence opportunistically.
        let _ = sqlx::query(
            r#"
            UPDATE stellar_submission_channels
            SET reserved_sequence = GREATEST(reserved_sequence, $1), total_submitted = total_submitted + 1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(reserved)
        .bind(channel.db_id)
        .execute(&self.pool)
        .await;

        channel
            .submission_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok((channel, reserved))
    }

    /// Reserve a sequence number, applying bounded backpressure: when every
    /// channel is at its in-flight cap, wait for one to free up (retrying
    /// with a short backoff) instead of failing the caller immediately.
    ///
    /// The pool's `queue` semaphore bounds how many callers may be waiting
    /// concurrently to `queue_depth` — once that's full, callers fail fast
    /// with [`SubmissionError::QueueTimeout`] rather than piling up
    /// unboundedly. A caller that does get a queue slot still gives up and
    /// returns [`SubmissionError::QueueTimeout`] if no channel frees up
    /// within `queue_timeout`.
    ///
    /// [`SubmissionError::NoActiveChannels`] (no channels registered, or
    /// every circuit breaker open) is not retried — waiting cannot help.
    pub async fn reserve_sequence_with_backpressure(&self) -> SubmissionResult<(ChannelHandle, i64)> {
        let start = Instant::now();

        let _permit = match timeout(self.queue_timeout, self.queue.acquire()).await {
            Ok(Ok(permit)) => permit,
            _ => {
                crate::metrics::stellar::queue_timeout_total()
                    .with_label_values(&[])
                    .inc();
                return Err(SubmissionError::QueueTimeout {
                    waited_ms: start.elapsed().as_millis() as u64,
                    timeout_ms: self.queue_timeout.as_millis() as u64,
                });
            }
        };
        let _depth_guard = QueueDepthGuard::new();

        loop {
            match self.reserve_sequence().await {
                Ok(result) => return Ok(result),
                Err(SubmissionError::ChannelExhausted(_)) => {
                    if start.elapsed() >= self.queue_timeout {
                        crate::metrics::stellar::queue_timeout_total()
                            .with_label_values(&[])
                            .inc();
                        return Err(SubmissionError::QueueTimeout {
                            waited_ms: start.elapsed().as_millis() as u64,
                            timeout_ms: self.queue_timeout.as_millis() as u64,
                        });
                    }
                    tokio::time::sleep(QUEUE_POLL_INTERVAL).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Rotate to next channel (used after errors)
    pub async fn rotate_channel(&self) -> SubmissionResult<()> {
        self.current_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Mark a channel as having failed
    pub async fn mark_channel_failure(&self, channel_id: Uuid) -> SubmissionResult<()> {
        let channels = self.channels.read().await;

        for channel in channels.iter() {
            if channel.db_id == channel_id {
                let mut cb_state = channel.circuit_breaker_state.lock().await;
                cb_state.consecutive_failures += 1;
                cb_state.last_failure_at = Some(Utc::now());

                if cb_state.consecutive_failures >= cb_state.threshold {
                    cb_state.is_open = true;
                }

                channel
                    .failure_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                // Update database
                sqlx::query(
                    r#"
                    UPDATE stellar_submission_channels
                    SET consecutive_failures = $1, last_error_at = NOW()
                    WHERE id = $2
                    "#,
                )
                .bind(cb_state.consecutive_failures as i32)
                .bind(channel_id)
                .execute(&self.pool)
                .await?;

                break;
            }
        }

        Ok(())
    }

    /// Mark a channel as succeeded (reset failure counter)
    pub async fn mark_channel_success(&self, channel_id: Uuid) -> SubmissionResult<()> {
        let channels = self.channels.read().await;

        for channel in channels.iter() {
            if channel.db_id == channel_id {
                let mut cb_state = channel.circuit_breaker_state.lock().await;
                cb_state.consecutive_failures = 0;
                cb_state.is_open = false;

                channel
                    .success_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                // Update database
                sqlx::query(
                    r#"
                    UPDATE stellar_submission_channels
                    SET consecutive_failures = 0, updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(channel_id)
                .execute(&self.pool)
                .await?;

                break;
            }
        }

        Ok(())
    }

    /// Mark sequence confirmed for channel to eliminate sequence contention.
    pub async fn mark_channel_confirmed_sequence(
        &self,
        channel_id: Uuid,
        confirmed_sequence: i64,
    ) -> SubmissionResult<()> {
        let channels = self.channels.read().await;
        for channel in channels.iter() {
            if channel.db_id == channel_id {
                let mut current = channel.sequence_counter.load(Ordering::SeqCst);
                while confirmed_sequence > current {
                    match channel.sequence_counter.compare_exchange(
                        current,
                        confirmed_sequence,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(actual) => {
                            current = actual;
                            if confirmed_sequence <= current {
                                break;
                            }
                        }
                    }
                }

                sqlx::query(
                    r#"
                    UPDATE stellar_submission_channels
                    SET current_sequence = GREATEST(current_sequence, $1),
                        total_successful = total_successful + 1,
                        updated_at = NOW()
                    WHERE id = $2
                    "#,
                )
                .bind(confirmed_sequence)
                .bind(channel_id)
                .execute(&self.pool)
                .await?;
                break;
            }
        }
        Ok(())
    }

    /// Get channel statistics
    pub async fn get_channel_stats(&self) -> SubmissionResult<Vec<ChannelStats>> {
        let channels = self.channels.read().await;
        let mut stats = Vec::new();

        for channel in channels.iter() {
            let cb_state = channel.circuit_breaker_state.lock().await;
            stats.push(ChannelStats {
                channel_id: channel.db_id,
                index: channel.index,
                account_id: channel.account_id.clone(),
                current_sequence: channel.sequence_counter.load(Ordering::SeqCst),
                reserved_sequence: channel.reserved_counter.load(Ordering::SeqCst),
                in_flight: channel.reserved_counter.load(Ordering::SeqCst)
                    - channel.sequence_counter.load(Ordering::SeqCst),
                total_submitted: channel.submission_count.load(Ordering::SeqCst),
                total_successful: channel.success_count.load(Ordering::SeqCst),
                total_failed: channel.failure_count.load(Ordering::SeqCst),
                consecutive_failures: cb_state.consecutive_failures,
                is_circuit_broken: cb_state.is_open,
            });
        }

        Ok(stats)
    }

    /// Get number of active channels
    pub async fn active_channel_count(&self) -> usize {
        self.channels.read().await.len()
    }

    /// Check channel pool capacity
    pub async fn get_pool_capacity_percent(&self) -> SubmissionResult<f64> {
        let stats = self.get_channel_stats().await?;

        if stats.is_empty() {
            return Ok(100.0);
        }

        let total_slots = stats.len() as i64 * self.max_in_flight_per_channel as i64;
        let used_slots: i64 = stats.iter().map(|s| s.in_flight).sum();

        Ok((used_slots as f64 / total_slots as f64) * 100.0)
    }
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: Uuid,
    pub index: i32,
    pub account_id: String,
    pub current_sequence: i64,
    pub reserved_sequence: i64,
    pub in_flight: i64,
    pub total_submitted: u64,
    pub total_successful: u64,
    pub total_failed: u64,
    pub consecutive_failures: u32,
    pub is_circuit_broken: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests require database
    #[tokio::test]
    async fn test_channel_selection() {
        // This would require a test database
        // Tested in integration tests
    }
}
