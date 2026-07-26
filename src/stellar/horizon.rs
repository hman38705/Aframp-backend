/// Horizon API client for transaction submission and confirmation polling
use crate::stellar::error::{HorizonErrorCode, SubmissionError, SubmissionResult};
use crate::stellar::models::HorizonTransaction;
use serde::Deserialize;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};

/// EWMA smoothing factor applied to each success/failure sample.
const EWMA_ALPHA: f64 = 0.2;
/// Minimum number of samples before an endpoint is eligible to be tripped
/// or recovered — avoids flapping a fresh endpoint on a single bad request.
const MIN_SAMPLES_BEFORE_TRIP: u32 = 5;
/// EWMA success rate below which a healthy endpoint is circuit-broken out
/// of rotation.
const UNHEALTHY_THRESHOLD: f64 = 0.5;
/// EWMA success rate at/above which a circuit-broken endpoint is re-added
/// to rotation.
const RECOVERY_THRESHOLD: f64 = 0.8;
/// How often the background prober re-checks circuit-broken endpoints.
const RECOVERY_PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// Per-endpoint health tracking state.
struct EndpointHealthState {
    /// Exponential moving average of the success rate, in `[0.0, 1.0]`.
    /// Starts optimistic (1.0) so a fresh endpoint isn't penalized before
    /// it has served any traffic.
    ewma_success_rate: f64,
    samples: u32,
    healthy: bool,
    consecutive_failures: u32,
}

impl EndpointHealthState {
    fn new() -> Self {
        Self {
            ewma_success_rate: 1.0,
            samples: 0,
            healthy: true,
            consecutive_failures: 0,
        }
    }
}

fn new_health_states(count: usize) -> Arc<Vec<Mutex<EndpointHealthState>>> {
    Arc::new((0..count).map(|_| Mutex::new(EndpointHealthState::new())).collect())
}

#[derive(Clone)]
pub struct HorizonClient {
    base_url: String,
    rpc_endpoints: std::sync::Arc<Vec<String>>,
    rr_index: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Per-endpoint health state, indexed identically to `rpc_endpoints`.
    endpoint_health: Arc<Vec<Mutex<EndpointHealthState>>>,
    client: reqwest::Client,
    request_timeout: Duration,
}

#[derive(Debug, Deserialize)]
pub struct HorizonErrorResponse {
    pub status: Option<u16>,
    pub type_url: Option<String>,
    pub title: Option<String>,
    pub detail: Option<String>,
    pub instance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionsResponse {
    pub _links: Option<serde_json::Value>,
    pub records: Vec<HorizonTransaction>,
}

impl HorizonClient {
    pub fn new(base_url: String) -> Self {
        let endpoints = vec![base_url.clone()];
        let endpoint_health = new_health_states(endpoints.len());
        Self {
            base_url,
            rpc_endpoints: std::sync::Arc::new(endpoints),
            rr_index: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            endpoint_health,
            client: reqwest::Client::new(),
            request_timeout: Duration::from_secs(15),
        }
    }

    pub fn with_rpc_endpoints(mut self, endpoints: Vec<String>) -> Self {
        if !endpoints.is_empty() {
            self.base_url = endpoints[0].clone();
            self.endpoint_health = new_health_states(endpoints.len());
            self.rpc_endpoints = std::sync::Arc::new(endpoints);
            self.rr_index.store(0, Ordering::Relaxed);

            if self.rpc_endpoints.len() > 1 {
                self.spawn_recovery_prober();
            }
        }
        self
    }

    /// Pick the next endpoint for round-robin dispatch, skipping any endpoint
    /// whose circuit breaker is open. Returns the endpoint's index (for
    /// recording the outcome via [`record_result`]) and URL.
    ///
    /// [`record_result`]: HorizonClient::record_result
    fn pick_endpoint(&self) -> (usize, String) {
        let endpoints = self.rpc_endpoints.as_ref();
        if endpoints.is_empty() {
            return (0, self.base_url.clone());
        }
        if endpoints.len() == 1 {
            return (0, endpoints[0].clone());
        }

        let mut candidates: Vec<usize> = self
            .endpoint_health
            .iter()
            .enumerate()
            .filter_map(|(i, h)| match h.lock() {
                Ok(state) if state.healthy => Some(i),
                _ => None,
            })
            .collect();

        if candidates.is_empty() {
            // Every endpoint is circuit-broken. Fail open across the full
            // set rather than hard-failing every submission — a total
            // Horizon outage should degrade, not stop, traffic.
            warn!("All Horizon RPC endpoints are circuit-broken; failing open across the full set");
            candidates = (0..endpoints.len()).collect();
        }

        let pick = self.rr_index.fetch_add(1, Ordering::Relaxed) % candidates.len();
        let idx = candidates[pick];
        (idx, endpoints[idx].clone())
    }

    /// Record the outcome of a request against `endpoint_idx`, updating its
    /// EWMA success rate and the Prometheus gauge, and flipping its circuit
    /// breaker open/closed as thresholds are crossed.
    fn record_result(&self, endpoint_idx: usize, success: bool) {
        let Some(slot) = self.endpoint_health.get(endpoint_idx) else {
            return;
        };
        let endpoint_label = self
            .rpc_endpoints
            .get(endpoint_idx)
            .cloned()
            .unwrap_or_else(|| self.base_url.clone());

        let (rate, healthy, just_tripped, just_recovered) = {
            let Ok(mut state) = slot.lock() else {
                return;
            };

            let sample = if success { 1.0 } else { 0.0 };
            state.ewma_success_rate =
                EWMA_ALPHA * sample + (1.0 - EWMA_ALPHA) * state.ewma_success_rate;
            state.samples = state.samples.saturating_add(1);
            state.consecutive_failures = if success {
                0
            } else {
                state.consecutive_failures.saturating_add(1)
            };

            let mut just_tripped = false;
            let mut just_recovered = false;

            if state.healthy
                && state.samples >= MIN_SAMPLES_BEFORE_TRIP
                && state.ewma_success_rate < UNHEALTHY_THRESHOLD
            {
                state.healthy = false;
                just_tripped = true;
            } else if !state.healthy && state.ewma_success_rate >= RECOVERY_THRESHOLD {
                state.healthy = true;
                state.consecutive_failures = 0;
                just_recovered = true;
            }

            (state.ewma_success_rate, state.healthy, just_tripped, just_recovered)
        };

        crate::metrics::stellar_horizon::endpoint_success_rate()
            .with_label_values(&[&endpoint_label])
            .set(rate);
        crate::metrics::stellar_horizon::endpoint_healthy()
            .with_label_values(&[&endpoint_label])
            .set(if healthy { 1.0 } else { 0.0 });

        if just_tripped {
            warn!(
                endpoint = %endpoint_label,
                success_rate = rate,
                "Horizon endpoint circuit-broken; excluded from rotation"
            );
        }
        if just_recovered {
            info!(
                endpoint = %endpoint_label,
                success_rate = rate,
                "Horizon endpoint recovered; re-added to rotation"
            );
        }
    }

    /// Spawn a background task that periodically probes circuit-broken
    /// endpoints with a lightweight GET, re-admitting them to rotation once
    /// their EWMA success rate recovers above [`RECOVERY_THRESHOLD`].
    fn spawn_recovery_prober(&self) {
        let handle = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(RECOVERY_PROBE_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                handle.run_recovery_probe_cycle().await;
            }
        });
    }

    async fn run_recovery_probe_cycle(&self) {
        let endpoints = self.rpc_endpoints.clone();
        for (idx, endpoint) in endpoints.iter().enumerate() {
            let is_unhealthy = self
                .endpoint_health
                .get(idx)
                .and_then(|m| m.lock().ok())
                .map(|state| !state.healthy)
                .unwrap_or(false);

            if !is_unhealthy {
                continue;
            }

            let probe_url = format!("{}/", endpoint.trim_end_matches('/'));
            let ok = self
                .client
                .get(&probe_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            debug_assert!(idx < endpoints.len());
            self.record_result(idx, ok);
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Submit a transaction to Horizon
    pub async fn submit_transaction(
        &self,
        tx_envelope: &str,
    ) -> SubmissionResult<HorizonTransaction> {
        let (endpoint_idx, endpoint) = self.pick_endpoint();
        let url = format!("{}/transactions", endpoint);

        let mut params = std::collections::HashMap::new();
        params.insert("tx", tx_envelope);

        let response = self
            .client
            .post(&url)
            .form(&params)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| {
                self.record_result(endpoint_idx, false);
                SubmissionError::HorizonApi(format!("POST /transactions failed: {}", e))
            })?;

        let status = response.status();
        // A response — even an application-level error like tx_bad_seq —
        // means the endpoint itself is up. Only network failures and 5xx
        // responses count against the endpoint's health.
        self.record_result(endpoint_idx, !status.is_server_error());

        if status.is_success() {
            let tx: HorizonTransaction = response.json().await.map_err(|e| {
                SubmissionError::HorizonApi(format!("failed to parse transaction response: {}", e))
            })?;
            Ok(tx)
        } else {
            let error_msg = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());

            // Try to parse as Horizon error
            if let Ok(horizon_err) = serde_json::from_str::<HorizonErrorResponse>(&error_msg) {
                let detail = horizon_err.detail.unwrap_or_default();
                let error_code = HorizonErrorCode::from_str(&detail);

                return Err(match error_code {
                    HorizonErrorCode::TxBadSeq => SubmissionError::BadSequence(detail),
                    HorizonErrorCode::TxInsufficientFee => SubmissionError::InsufficientFee {
                        provided: 0,
                        required: 0,
                    },
                    HorizonErrorCode::TxMalformed => SubmissionError::MalformedTransaction(detail),
                    _ if error_code.is_retryable() => SubmissionError::TransientNetworkError {
                        source: detail,
                        attempt: 1,
                    },
                    _ => SubmissionError::UnknownHorizonError {
                        code: status.to_string(),
                        message: detail,
                    },
                });
            }

            Err(SubmissionError::HorizonApi(format!(
                "submission failed ({}): {}",
                status, error_msg
            )))
        }
    }

    /// Get transaction by hash from Horizon
    pub async fn get_transaction(
        &self,
        tx_hash: &str,
    ) -> SubmissionResult<Option<HorizonTransaction>> {
        let (endpoint_idx, endpoint) = self.pick_endpoint();
        let url = format!("{}/transactions/{}", endpoint, tx_hash);

        let response = self
            .client
            .get(&url)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| {
                self.record_result(endpoint_idx, false);
                SubmissionError::HorizonApi(format!("GET /transactions/{{}} failed: {}", e))
            })?;

        self.record_result(endpoint_idx, !response.status().is_server_error());

        if response.status() == 404 {
            return Ok(None);
        }

        if response.status().is_success() {
            let tx: HorizonTransaction = response.json().await.map_err(|e| {
                SubmissionError::HorizonApi(format!("failed to parse transaction response: {}", e))
            })?;
            Ok(Some(tx))
        } else {
            Err(SubmissionError::HorizonApi(format!(
                "failed to fetch transaction: {}",
                response.status()
            )))
        }
    }

    /// Get account details including current sequence
    pub async fn get_account_sequence(&self, account_id: &str) -> SubmissionResult<i64> {
        let (endpoint_idx, endpoint) = self.pick_endpoint();
        let url = format!("{}/accounts/{}", endpoint, account_id);

        #[derive(Deserialize)]
        struct AccountResponse {
            sequence: String,
        }

        let response = self
            .client
            .get(&url)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| {
                self.record_result(endpoint_idx, false);
                SubmissionError::HorizonApi(format!("failed to fetch account sequence: {}", e))
            })?;

        self.record_result(endpoint_idx, !response.status().is_server_error());

        if response.status().is_success() {
            let account: AccountResponse = response.json().await.map_err(|e| {
                SubmissionError::HorizonApi(format!("failed to parse account response: {}", e))
            })?;

            account
                .sequence
                .parse::<i64>()
                .map_err(|_| SubmissionError::HorizonApi("invalid sequence format".to_string()))
        } else if response.status() == 404 {
            Err(SubmissionError::HorizonApi(format!(
                "account {} not found",
                account_id
            )))
        } else {
            Err(SubmissionError::HorizonApi(format!(
                "failed to fetch account: {}",
                response.status()
            )))
        }
    }

    /// Poll for transaction confirmation (exponential backoff)
    pub async fn poll_transaction_confirmation(
        &self,
        tx_hash: &str,
        max_attempts: u32,
    ) -> SubmissionResult<Option<HorizonTransaction>> {
        let mut backoff_ms = 100u64;
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self.get_transaction(tx_hash).await? {
                Some(tx) => return Ok(Some(tx)),
                None if attempt >= max_attempts => return Ok(None),
                None => {
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(5000); // Cap at 5s
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HorizonClient::new("https://horizon-testnet.stellar.org".to_string());
        assert_eq!(client.base_url, "https://horizon-testnet.stellar.org");
    }

    #[test]
    fn test_client_with_timeout() {
        let client = HorizonClient::new("https://horizon-testnet.stellar.org".to_string())
            .with_timeout(Duration::from_secs(30));
        assert_eq!(client.request_timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_endpoint_circuit_breaks_after_repeated_failures() {
        let client = HorizonClient::new("https://a.example.com".to_string()).with_rpc_endpoints(
            vec!["https://a.example.com".to_string(), "https://b.example.com".to_string()],
        );

        for _ in 0..MIN_SAMPLES_BEFORE_TRIP {
            client.record_result(0, false);
        }

        assert!(!client.endpoint_health[0].lock().unwrap().healthy);
        assert!(client.endpoint_health[1].lock().unwrap().healthy);

        // Rotation must now exclusively favor the healthy endpoint.
        for _ in 0..10 {
            let (idx, _) = client.pick_endpoint();
            assert_eq!(idx, 1, "unhealthy endpoint 0 must be excluded from rotation");
        }
    }

    #[tokio::test]
    async fn test_endpoint_recovers_after_success_streak() {
        let client = HorizonClient::new("https://a.example.com".to_string()).with_rpc_endpoints(
            vec!["https://a.example.com".to_string(), "https://b.example.com".to_string()],
        );

        for _ in 0..MIN_SAMPLES_BEFORE_TRIP {
            client.record_result(0, false);
        }
        assert!(!client.endpoint_health[0].lock().unwrap().healthy);

        for _ in 0..10 {
            client.record_result(0, true);
        }

        assert!(
            client.endpoint_health[0].lock().unwrap().healthy,
            "endpoint should recover once its EWMA success rate crosses the recovery threshold"
        );

        // Both endpoints should now be eligible for rotation.
        let mut seen_zero = false;
        for _ in 0..20 {
            let (idx, _) = client.pick_endpoint();
            if idx == 0 {
                seen_zero = true;
            }
        }
        assert!(seen_zero, "recovered endpoint should be back in rotation");
    }

    #[tokio::test]
    async fn test_fails_open_when_all_endpoints_unhealthy() {
        let client = HorizonClient::new("https://a.example.com".to_string()).with_rpc_endpoints(
            vec!["https://a.example.com".to_string(), "https://b.example.com".to_string()],
        );

        for idx in 0..2 {
            for _ in 0..MIN_SAMPLES_BEFORE_TRIP {
                client.record_result(idx, false);
            }
        }
        assert!(!client.endpoint_health[0].lock().unwrap().healthy);
        assert!(!client.endpoint_health[1].lock().unwrap().healthy);

        // Every endpoint is circuit-broken — must fail open rather than panic
        // or return an invalid index.
        let (idx, _) = client.pick_endpoint();
        assert!(idx < 2);
    }

    #[test]
    fn test_single_endpoint_never_excluded_from_rotation() {
        let client = HorizonClient::new("https://horizon-testnet.stellar.org".to_string());
        for _ in 0..MIN_SAMPLES_BEFORE_TRIP {
            client.record_result(0, false);
        }
        // A lone endpoint can't be rotated away from — pick_endpoint must
        // still return it.
        let (idx, url) = client.pick_endpoint();
        assert_eq!(idx, 0);
        assert_eq!(url, "https://horizon-testnet.stellar.org");
    }
}
