use crate::chains::stellar::client::{HorizonTransactionRecord, StellarClient};
use crate::database::repository::Repository;
use crate::database::transaction_repository::TransactionRepository;
use crate::database::webhook_repository::WebhookRepository;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TransactionMonitorConfig {
    pub poll_interval: Duration,
    pub pending_timeout: Duration,
    pub max_retries: u32,
    pub pending_batch_size: i64,
    pub incoming_limit: usize,
    pub system_wallet_address: Option<String>,
}

impl Default for TransactionMonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(7),
            pending_timeout: Duration::from_secs(600),
            max_retries: 3,
            pending_batch_size: 200,
            incoming_limit: 100,
            system_wallet_address: None,
        }
    }
}

impl TransactionMonitorConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        cfg.poll_interval = Duration::from_secs(
            std::env::var("TX_MONITOR_POLL_INTERVAL_SECONDS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(cfg.poll_interval.as_secs()),
        );
        cfg.pending_timeout = Duration::from_secs(
            std::env::var("TX_MONITOR_PENDING_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(cfg.pending_timeout.as_secs()),
        );
        cfg.max_retries = std::env::var("TX_MONITOR_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(cfg.max_retries);
        cfg.pending_batch_size = std::env::var("TX_MONITOR_PENDING_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(cfg.pending_batch_size);
        cfg.incoming_limit = std::env::var("TX_MONITOR_INCOMING_LIMIT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(cfg.incoming_limit);
        cfg.system_wallet_address = std::env::var("SYSTEM_WALLET_ADDRESS").ok();
        cfg
    }
}

pub struct TransactionMonitorWorker {
    pool: PgPool,
    stellar_client: StellarClient,
    config: TransactionMonitorConfig,
    incoming_cursor: Option<String>,
}

impl TransactionMonitorWorker {
    pub fn new(
        pool: PgPool,
        stellar_client: StellarClient,
        config: TransactionMonitorConfig,
    ) -> Self {
        Self {
            pool,
            stellar_client,
            config,
            incoming_cursor: None,
        }
    }

    pub async fn run(mut self, mut shutdown_rx: watch::Receiver<bool>) {
        info!(
            poll_interval_secs = self.config.poll_interval.as_secs(),
            pending_timeout_secs = self.config.pending_timeout.as_secs(),
            max_retries = self.config.max_retries,
            has_system_wallet = self.config.system_wallet_address.is_some(),
            "stellar transaction monitor worker started"
        );

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("stellar transaction monitor worker stopping");
                        break;
                    }
                }
                _ = tokio::time::sleep(self.config.poll_interval) => {
                    if let Err(e) = self.run_cycle().await {
                        warn!(error = %e, "transaction monitor cycle failed");
                    }
                }
            }
        }

        info!("stellar transaction monitor worker stopped");
    }

    async fn run_cycle(&mut self) -> anyhow::Result<()> {
        self.process_pending_transactions().await?;
        self.scan_incoming_transactions().await?;
        Ok(())
    }

    async fn process_pending_transactions(&self) -> anyhow::Result<()> {
        let tx_repo = TransactionRepository::new(self.pool.clone());
        let pending = tx_repo
            .find_pending_payments_for_monitoring(self.config.pending_batch_size as i32)
            .await?;

        for tx in pending {
            let tx_hash = extract_tx_hash(Some(&tx.metadata));
            if tx_hash.is_none() {
                warn!(transaction_id = %tx.transaction_id, "pending transaction has no stellar hash in metadata");
                continue;
            }
            let tx_hash = tx_hash.unwrap_or_default();

            if is_timed_out(tx.updated_at, self.config.pending_timeout) {
                self.handle_timeout(&tx.transaction_id.to_string(), Some(tx.metadata.clone()))
                    .await?;
                continue;
            }

            match self.stellar_client.get_transaction_by_hash(&tx_hash).await {
                Ok(record) => {
                    self.handle_horizon_status(
                        &tx.transaction_id.to_string(),
                        record,
                        Some(tx.metadata.clone()),
                    )
                    .await?;
                }
                Err(e) => {
                    let message = e.to_string().to_lowercase();
                    // Keep pending on temporary/network states.
                    if message.contains("not found")
                        || message.contains("network")
                        || message.contains("timeout")
                        || message.contains("rate limit")
                    {
                        continue;
                    }
                    self.fail_or_retry(
                        &tx.transaction_id.to_string(),
                        Some(tx.metadata.clone()),
                        &e.to_string(),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_horizon_status(
        &self,
        transaction_id: &str,
        record: HorizonTransactionRecord,
        metadata: Option<JsonValue>,
    ) -> anyhow::Result<()> {
        let mut updated = metadata.unwrap_or_else(|| json!({}));
        merge_status_fields(&mut updated, &record);

        let tx_repo = TransactionRepository::new(self.pool.clone());
        if record.successful {
            tx_repo
                .update_status_with_metadata(transaction_id, "completed", updated.clone())
                .await?;
            self.log_webhook_event(transaction_id, "stellar.transaction.confirmed", updated)
                .await;
        } else {
            let reason = record
                .result_xdr
                .as_deref()
                .unwrap_or("transaction failed on horizon");
            self.fail_or_retry(transaction_id, Some(updated), reason)
                .await?;
        }
        Ok(())
    }

    async fn handle_timeout(
        &self,
        transaction_id: &str,
        metadata: Option<JsonValue>,
    ) -> anyhow::Result<()> {
        let retries = next_retry_count(metadata.as_ref());
        let mut updated = metadata.unwrap_or_else(|| json!({}));
        updated["last_monitor_error"] = json!("pending timeout exceeded");
        updated["retry_count"] = json!(retries);
        updated["last_retry_at"] = json!(chrono::Utc::now().to_rfc3339());

        let tx_repo = TransactionRepository::new(self.pool.clone());
        if retries <= self.config.max_retries {
            tx_repo
                .update_status_with_metadata(transaction_id, "pending", updated.clone())
                .await?;
            warn!(
                transaction_id = %transaction_id,
                retry_count = retries,
                "transaction timed out, queued for retry"
            );
        } else {
            tx_repo
                .update_status_with_metadata(transaction_id, "failed", updated.clone())
                .await?;
            self.log_webhook_event(transaction_id, "stellar.transaction.timeout", updated)
                .await;
            warn!(
                transaction_id = %transaction_id,
                retry_count = retries,
                "transaction timed out and exceeded max retries"
            );
        }
        Ok(())
    }

    async fn fail_or_retry(
        &self,
        transaction_id: &str,
        metadata: Option<JsonValue>,
        error_message: &str,
    ) -> anyhow::Result<()> {
        let retries = next_retry_count(metadata.as_ref());
        let retryable = is_retryable_error(error_message);
        let mut updated = metadata.unwrap_or_else(|| json!({}));
        updated["last_monitor_error"] = json!(error_message);
        updated["retry_count"] = json!(retries);
        updated["last_retry_at"] = json!(chrono::Utc::now().to_rfc3339());
        updated["retryable"] = json!(retryable);

        let tx_repo = TransactionRepository::new(self.pool.clone());
        if retryable && retries <= self.config.max_retries {
            tx_repo
                .update_status_with_metadata(transaction_id, "pending", updated.clone())
                .await?;
            warn!(
                transaction_id = %transaction_id,
                retry_count = retries,
                error = %error_message,
                "transaction failed with retryable error; keeping pending"
            );
        } else {
            tx_repo
                .update_status_with_metadata(transaction_id, "failed", updated.clone())
                .await?;
            self.log_webhook_event(transaction_id, "stellar.transaction.failed", updated)
                .await;
            warn!(
                transaction_id = %transaction_id,
                retry_count = retries,
                error = %error_message,
                "transaction marked failed"
            );
        }
        Ok(())
    }

    async fn scan_incoming_transactions(&mut self) -> anyhow::Result<()> {
        let system_wallet = match self.config.system_wallet_address.as_deref() {
            Some(addr) => addr,
            None => return Ok(()),
        };

        let page = self
            .stellar_client
            .list_account_transactions(
                system_wallet,
                self.config.incoming_limit,
                self.incoming_cursor.as_deref(),
            )
            .await?;

        let mut newest_cursor = self.incoming_cursor.clone();
        for tx in page.records {
            if !tx.successful {
                continue;
            }

            if let Some(cursor) = tx.paging_token.clone() {
                newest_cursor = Some(cursor);
            }

            let memo = match tx.memo.as_deref() {
                Some(m) if !m.trim().is_empty() => m,
                _ => continue,
            };

            let looks_like_incoming = self
                .is_incoming_cngn_payment(&tx.hash, system_wallet)
                .await
                .unwrap_or(false);
            if !looks_like_incoming {
                continue;
            }

            let tx_repo = TransactionRepository::new(self.pool.clone());
            match tx_repo.find_by_id(memo).await {
                Ok(Some(db_tx)) if db_tx.status == "pending" || db_tx.status == "processing" => {
                    let mut metadata = db_tx.metadata.clone();
                    metadata["incoming_hash"] = json!(tx.hash);
                    metadata["incoming_ledger"] = json!(tx.ledger);
                    metadata["incoming_confirmed_at"] = json!(chrono::Utc::now().to_rfc3339());
                    tx_repo
                        .update_status_with_metadata(
                            &db_tx.transaction_id.to_string(),
                            "completed",
                            metadata.clone(),
                        )
                        .await?;
                    self.log_webhook_event(
                        &db_tx.transaction_id.to_string(),
                        "stellar.incoming.matched",
                        metadata,
                    )
                    .await;
                }
                Ok(_) => {
                    self.log_unmatched_incoming(memo, &tx).await;
                }
                Err(e) => {
                    warn!(
                        memo = %memo,
                        error = %e,
                        "failed to look up memo for incoming transaction"
                    );
                }
            }
        }

        self.incoming_cursor = newest_cursor;
        Ok(())
    }

    async fn is_incoming_cngn_payment(
        &self,
        tx_hash: &str,
        system_wallet: &str,
    ) -> anyhow::Result<bool> {
        let issuer = std::env::var("CNGN_ISSUER_TESTNET")
            .or_else(|_| std::env::var("CNGN_ISSUER_MAINNET"))
            .unwrap_or_default();
        let operations = self
            .stellar_client
            .get_transaction_operations(tx_hash)
            .await?;
        for op in operations {
            let op_type = op.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if op_type != "payment" {
                continue;
            }

            let destination = op.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let asset_code = op.get("asset_code").and_then(|v| v.as_str()).unwrap_or("");
            let asset_issuer = op
                .get("asset_issuer")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if destination == system_wallet && asset_code.eq_ignore_ascii_case("cngn") {
                if issuer.is_empty() || asset_issuer == issuer {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    async fn log_webhook_event(&self, transaction_id: &str, event_type: &str, payload: JsonValue) {
        let parsed_tx_id = Uuid::parse_str(transaction_id).ok();
        let repo = WebhookRepository::new(self.pool.clone());
        let event_id = format!("{}:{}", event_type, transaction_id);
        if let Err(e) = repo
            .log_event(
                &event_id,
                "stellar",
                event_type,
                payload,
                None,
                parsed_tx_id,
            )
            .await
        {
            warn!(
                transaction_id = %transaction_id,
                event_type = %event_type,
                error = %e,
                "failed to write webhook event"
            );
        }
    }

    async fn log_unmatched_incoming(&self, memo: &str, tx: &HorizonTransactionRecord) {
        let repo = WebhookRepository::new(self.pool.clone());
        let event_id = format!("unmatched:{}", tx.hash);
        let payload = json!({
            "memo": memo,
            "hash": tx.hash,
            "ledger": tx.ledger,
            "created_at": tx.created_at,
        });
        if let Err(e) = repo
            .log_event(
                &event_id,
                "stellar",
                "stellar.incoming.unmatched",
                payload,
                None,
                None,
            )
            .await
        {
            error!(error = %e, "failed to log unmatched incoming transaction");
        }
    }
}

fn extract_tx_hash(metadata: Option<&JsonValue>) -> Option<String> {
    let metadata = metadata?;
    for key in [
        "submitted_hash",
        "stellar_tx_hash",
        "transaction_hash",
        "hash",
    ] {
        if let Some(value) = metadata.get(key).and_then(|v| v.as_str()) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn next_retry_count(metadata: Option<&JsonValue>) -> u32 {
    metadata
        .and_then(|m| m.get("retry_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
        + 1
}

fn is_timed_out(last_updated: chrono::DateTime<chrono::Utc>, timeout: Duration) -> bool {
    let elapsed = chrono::Utc::now() - last_updated;
    elapsed.to_std().map(|d| d > timeout).unwrap_or(false)
}

fn is_retryable_error(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("tx_bad_seq")
        || m.contains("bad sequence")
        || m.contains("tx_insufficient_fee")
        || m.contains("insufficient fee")
        || m.contains("timeout")
        || m.contains("rate limit")
        || m.contains("network")
}

fn merge_status_fields(metadata: &mut JsonValue, record: &HorizonTransactionRecord) {
    metadata["submitted_hash"] = json!(record.hash);
    metadata["confirmed_ledger"] = json!(record.ledger);
    metadata["confirmed_at"] = json!(record.created_at);
    metadata["horizon_successful"] = json!(record.successful);
    if let Some(result_xdr) = &record.result_xdr {
        metadata["result_xdr"] = json!(result_xdr);
    }
    if let Some(result_meta_xdr) = &record.result_meta_xdr {
        metadata["result_meta_xdr"] = json!(result_meta_xdr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> HorizonTransactionRecord {
        HorizonTransactionRecord {
            id: Some("id_1".to_string()),
            paging_token: Some("123".to_string()),
            hash: "stellar_hash_1".to_string(),
            successful: true,
            ledger: Some(9876),
            created_at: Some("2026-02-12T00:00:00Z".to_string()),
            memo_type: Some("text".to_string()),
            memo: Some("memo-1".to_string()),
            result_xdr: Some("result_xdr_1".to_string()),
            result_meta_xdr: Some("result_meta_xdr_1".to_string()),
            envelope_xdr: Some("envelope_xdr_1".to_string()),
            fee_charged: Some("100".to_string()),
        }
    }

    #[test]
    fn retryable_error_codes_are_detected() {
        assert!(is_retryable_error("tx_bad_seq"));
        assert!(is_retryable_error("tx_insufficient_fee"));
        assert!(is_retryable_error("network error while checking horizon"));
        assert!(is_retryable_error("rate limit exceeded"));
        assert!(!is_retryable_error("op_underfunded"));
    }

    #[test]
    fn hash_extraction_uses_known_keys() {
        let meta = json!({"transaction_hash": "abc"});
        assert_eq!(extract_tx_hash(Some(&meta)).as_deref(), Some("abc"));
    }

    #[test]
    fn hash_extraction_prefers_submitted_hash_first() {
        let meta = json!({
            "submitted_hash": "preferred",
            "stellar_tx_hash": "secondary",
            "transaction_hash": "tertiary"
        });
        assert_eq!(extract_tx_hash(Some(&meta)).as_deref(), Some("preferred"));
    }

    #[test]
    fn hash_extraction_ignores_empty_values() {
        let meta = json!({
            "submitted_hash": "",
            "stellar_tx_hash": "",
            "transaction_hash": "fallback"
        });
        assert_eq!(extract_tx_hash(Some(&meta)).as_deref(), Some("fallback"));
    }

    #[test]
    fn retry_count_defaults_to_one() {
        assert_eq!(next_retry_count(None), 1);
        let meta = json!({});
        assert_eq!(next_retry_count(Some(&meta)), 1);
    }

    #[test]
    fn retry_count_increments_existing_value() {
        let meta = json!({ "retry_count": 4 });
        assert_eq!(next_retry_count(Some(&meta)), 5);
    }

    #[test]
    fn timeout_detection_is_correct() {
        let now = chrono::Utc::now();
        let very_recent = now - chrono::Duration::seconds(5);
        let old = now - chrono::Duration::seconds(120);

        assert!(!is_timed_out(very_recent, Duration::from_secs(30)));
        assert!(is_timed_out(old, Duration::from_secs(30)));
    }

    #[test]
    fn merge_status_fields_copies_core_tracking_values() {
        let mut metadata = json!({});
        let record = sample_record();

        merge_status_fields(&mut metadata, &record);

        assert_eq!(metadata["submitted_hash"], json!("stellar_hash_1"));
        assert_eq!(metadata["confirmed_ledger"], json!(9876));
        assert_eq!(metadata["confirmed_at"], json!("2026-02-12T00:00:00Z"));
        assert_eq!(metadata["horizon_successful"], json!(true));
        assert_eq!(metadata["result_xdr"], json!("result_xdr_1"));
        assert_eq!(metadata["result_meta_xdr"], json!("result_meta_xdr_1"));
    }

    #[test]
    fn merge_status_fields_skips_optional_xdr_when_missing() {
        let mut metadata = json!({});
        let mut record = sample_record();
        record.result_xdr = None;
        record.result_meta_xdr = None;

        merge_status_fields(&mut metadata, &record);

        assert!(metadata.get("result_xdr").is_none());
        assert!(metadata.get("result_meta_xdr").is_none());
        assert_eq!(metadata["submitted_hash"], json!("stellar_hash_1"));
    }
}
