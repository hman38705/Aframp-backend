use crate::travel_rule::metrics;
use crate::travel_rule::repository::TravelRuleRepository;
use std::sync::Arc;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{error, info, warn};

const POLL_INTERVAL_SECS: u64 = 60;
/// Emit an alert log when this many messages are unacknowledged
const UNACKNOWLEDGED_ALERT_THRESHOLD: i64 = 10;

pub struct TravelRuleSlaWorker {
    repo: Arc<TravelRuleRepository>,
}

impl TravelRuleSlaWorker {
    pub fn new(repo: Arc<TravelRuleRepository>) -> Self {
        Self { repo }
    }

    /// Spawn as a background tokio task.
    pub fn start(self) {
        tokio::spawn(async move {
            let mut ticker = interval(TokioDuration::from_secs(POLL_INTERVAL_SECS));
            loop {
                ticker.tick().await;
                self.run_cycle().await;
            }
        });
    }

    async fn run_cycle(&self) {
        // 1. Expire SLA-breached exchanges
        let breached = match self.repo.get_unacknowledged_past_sla().await {
            Ok(b) => b,
            Err(e) => {
                error!(error = %e, "Travel Rule SLA worker: failed to fetch breached exchanges");
                return;
            }
        };

        for exchange in &breached {
            if let Err(e) = self.repo.mark_sla_breached(exchange.exchange_id).await {
                error!(
                    exchange_id = %exchange.exchange_id,
                    error = %e,
                    "Travel Rule SLA worker: failed to mark exchange as timed out"
                );
            } else {
                warn!(
                    exchange_id = %exchange.exchange_id,
                    vasp_id = %exchange.beneficiary_vasp_id,
                    transaction_id = %exchange.transaction_id,
                    "Travel Rule SLA BREACH: outbound message unacknowledged past SLA window"
                );
            }
        }

        if !breached.is_empty() {
            info!(count = breached.len(), "Travel Rule SLA worker: marked exchanges as timed_out");
        }

        // 2. Update gauges
        if let Ok(unack) = self.repo.count_unacknowledged_outbound().await {
            metrics::UNACKNOWLEDGED_OUTBOUND_COUNT.set(unack as f64);

            if unack >= UNACKNOWLEDGED_ALERT_THRESHOLD {
                warn!(
                    count = unack,
                    threshold = UNACKNOWLEDGED_ALERT_THRESHOLD,
                    "ALERT: Travel Rule unacknowledged outbound message count exceeds threshold"
                );
            }
        }

        if let Ok(pending) = self.repo.count_pending_inbound_screening().await {
            metrics::PENDING_INBOUND_SCREENING_COUNT.set(pending as f64);
        }

        if let Ok(size) = self.repo.vasp_registry_size().await {
            metrics::VASP_REGISTRY_SIZE.set(size as f64);
        }

        // 3. Check screening failure rate alert
        self.check_screening_failure_rate().await;
    }

    async fn check_screening_failure_rate(&self) {
        let from = chrono::Utc::now() - chrono::Duration::hours(24);
        let to = chrono::Utc::now();

        let metrics_result = self.repo.compute_metrics(from, to).await;
        match metrics_result {
            Ok(m) if m.inbound_received > 0 => {
                let failure_rate =
                    m.inbound_screening_failures as f64 / m.inbound_received as f64;
                if failure_rate > 0.10 {
                    warn!(
                        failure_rate = %format!("{:.1}%", failure_rate * 100.0),
                        inbound_total = m.inbound_received,
                        failures = m.inbound_screening_failures,
                        "ALERT: Travel Rule originator screening failure rate exceeds 10% in last 24h"
                    );
                }
            }
            Err(e) => {
                error!(error = %e, "Travel Rule SLA worker: metrics computation failed");
            }
            _ => {}
        }
    }
}
