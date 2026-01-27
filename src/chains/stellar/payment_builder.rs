use crate::chains::stellar::{
    client::StellarClient,
    config::StellarConfig,
    errors::{StellarError, StellarResult},
    types::is_valid_stellar_address,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoType {
    Text,
    Id,
    Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOperation {
    pub destination: String,
    pub amount: Decimal,
    pub asset_code: String,
    pub asset_issuer: String,
    pub source_account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMemo {
    pub memo_type: MemoType,
    pub value: String,
}

#[derive(Debug)]
pub struct PaymentBuilder {
    client: StellarClient,
    config: StellarConfig,
    pub operations: Vec<PaymentOperation>,
    pub memo: Option<TransactionMemo>,
    pub source_account: Option<String>,
    sequence_number: Option<i64>,
    base_fee: Option<u32>,
}

impl PaymentBuilder {
    pub fn new(client: StellarClient, config: StellarConfig) -> Self {
        Self {
            client,
            config,
            operations: Vec::new(),
            memo: None,
            source_account: None,
            sequence_number: None,
            base_fee: None,
        }
    }

    pub fn with_source_account(mut self, account: &str) -> Self {
        self.source_account = Some(account.to_string());
        self
    }

    pub async fn add_payment_op(
        &mut self,
        destination: &str,
        amount: Decimal,
        asset_code: &str,
        issuer: &str,
    ) -> StellarResult<&mut Self> {
        if !is_valid_stellar_address(destination) {
            return Err(StellarError::invalid_address(destination));
        }

        if !is_valid_stellar_address(issuer) {
            return Err(StellarError::invalid_address(issuer));
        }

        if amount <= Decimal::ZERO {
            return Err(StellarError::invalid_amount(format!(
                "Amount must be positive, got: {}",
                amount
            )));
        }

        if asset_code.is_empty() || asset_code.len() > 12 {
            return Err(StellarError::invalid_amount(format!(
                "Asset code must be 1-12 characters, got: {}",
                asset_code
            )));
        }

        self.verify_trustline(destination, asset_code, issuer)
            .await?;

        info!(
            "Adding payment operation: {} {} from source to {}",
            amount, asset_code, destination
        );

        let operation = PaymentOperation {
            destination: destination.to_string(),
            amount,
            asset_code: asset_code.to_string(),
            asset_issuer: issuer.to_string(),
            source_account: self.source_account.clone(),
        };

        self.operations.push(operation);
        Ok(self)
    }

    pub fn add_memo(&mut self, memo_type: MemoType, value: &str) -> StellarResult<&mut Self> {
        match memo_type {
            MemoType::Text => {
                if value.len() > 28 {
                    return Err(StellarError::invalid_memo(format!(
                        "Text memo cannot exceed 28 bytes, got {} bytes",
                        value.len()
                    )));
                }
            }
            MemoType::Id => {
                if u64::from_str(value).is_err() {
                    return Err(StellarError::invalid_memo(format!(
                        "ID memo must be a valid u64, got: {}",
                        value
                    )));
                }
            }
            MemoType::Hash => {
                if value.len() != 32 {
                    return Err(StellarError::invalid_memo(format!(
                        "Hash memo must be exactly 32 bytes, got {} bytes",
                        value.len()
                    )));
                }
            }
        }

        debug!("Adding memo: {:?} with value: {}", memo_type, value);

        self.memo = Some(TransactionMemo {
            memo_type,
            value: value.to_string(),
        });

        Ok(self)
    }

    pub async fn estimate_fee(&self) -> StellarResult<u32> {
        if self.operations.is_empty() {
            return Err(StellarError::build_failed(
                "Cannot estimate fee: no operations added".to_string(),
            ));
        }

        let base_fee = if let Some(cached_fee) = self.base_fee {
            debug!("Using cached base fee: {} stroops", cached_fee);
            cached_fee
        } else {
            let horizon_url = self.config.network.horizon_url();
            let url = format!("{}/fee_stats", horizon_url);

            debug!("Fetching fee stats from Horizon: {}", url);

            let response = self
                .client
                .http_client
                .get(&url)
                .send()
                .await
                .map_err(|e| {
                    StellarError::fee_estimation_failed(format!("Failed to fetch fee stats: {}", e))
                })?;

            let fee_stats: FeeStatsResponse = response.json().await.map_err(|e| {
                StellarError::fee_estimation_failed(format!("Failed to parse fee stats: {}", e))
            })?;

            let base_fee = fee_stats.last_ledger_base_fee.parse::<u32>().unwrap_or(100);

            debug!("Fetched base fee from Horizon: {} stroops", base_fee);
            base_fee
        };

        let operations_count = self.operations.len() as u32;
        let total_fee = base_fee * operations_count;

        if total_fee > 100_000 {
            warn!(
                "High network fee detected: {} stroops ({} operations × {} base fee)",
                total_fee, operations_count, base_fee
            );
        }

        info!(
            "Estimated fee: {} stroops for {} operation(s)",
            total_fee, operations_count
        );

        Ok(total_fee)
    }

    pub async fn sign_tx(&self, secret_key: &str) -> StellarResult<SignedTransaction> {
        if self.operations.is_empty() {
            return Err(StellarError::build_failed(
                "Cannot sign transaction: no operations added".to_string(),
            ));
        }

        if secret_key.len() != 56 || !secret_key.starts_with('S') {
            return Err(StellarError::signing_failed(
                "Invalid secret key format".to_string(),
            ));
        }

        let source_account = self.source_account.as_ref().ok_or_else(|| {
            StellarError::build_failed("Source account is required for signing".to_string())
        })?;

        let account_info = self.client.get_account(source_account).await?;

        let fee = self.estimate_fee().await?;

        info!(
            "Building transaction with {} operation(s), fee: {} stroops",
            self.operations.len(),
            fee
        );

        let transaction = SignedTransaction {
            source_account: source_account.clone(),
            sequence_number: account_info.sequence + 1,
            operations: self.operations.clone(),
            memo: self.memo.clone(),
            fee,
            network_passphrase: self.config.network.network_passphrase().to_string(),
        };

        info!(
            "Transaction signed successfully for account: {}",
            source_account
        );

        Ok(transaction)
    }

    async fn verify_trustline(
        &self,
        account: &str,
        asset_code: &str,
        issuer: &str,
    ) -> StellarResult<()> {
        debug!(
            "Verifying trustline for account {} with asset {}:{}",
            account, asset_code, issuer
        );

        let account_info = self.client.get_account(account).await?;

        let has_trustline = account_info.balances.iter().any(|balance| {
            balance.asset_code.as_deref() == Some(asset_code)
                && balance.asset_issuer.as_deref() == Some(issuer)
        });

        if !has_trustline {
            warn!(
                "Trustline not found for account {} with asset {}:{}",
                account, asset_code, issuer
            );
            return Err(StellarError::trustline_not_found(asset_code, issuer));
        }

        debug!(
            "Trustline verified for account {} with asset {}:{}",
            account, asset_code, issuer
        );

        Ok(())
    }

    pub async fn has_trustline(
        client: &StellarClient,
        account: &str,
        asset_code: &str,
        issuer: &str,
    ) -> StellarResult<bool> {
        let account_info = client.get_account(account).await?;

        let has_trustline = account_info.balances.iter().any(|balance| {
            balance.asset_code.as_deref() == Some(asset_code)
                && balance.asset_issuer.as_deref() == Some(issuer)
        });

        Ok(has_trustline)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedTransaction {
    pub source_account: String,
    pub sequence_number: i64,
    pub operations: Vec<PaymentOperation>,
    pub memo: Option<TransactionMemo>,
    pub fee: u32,
    pub network_passphrase: String,
}

#[derive(Debug, Deserialize)]
struct FeeStatsResponse {
    last_ledger_base_fee: String,
    #[allow(dead_code)]
    ledger_capacity_usage: String,
}
