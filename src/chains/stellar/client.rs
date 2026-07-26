//! Compatibility shim for the historical `chains::stellar::client::StellarClient`.
//!
//! Wraps [`crate::stellar::horizon::HorizonClient`] and, for endpoints that
//! client doesn't expose, talks to Horizon directly. See [`super`] for
//! background on why this exists.

use super::config::StellarConfig;
use super::errors::StellarError;
use crate::stellar::horizon::HorizonClient;
use crate::stellar::models::HorizonTransaction;
use serde::Deserialize;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct StellarHealthStatus {
    pub is_healthy: bool,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StellarBalance {
    pub balance: String,
    pub asset_type: String,
    #[serde(default)]
    pub asset_code: Option<String>,
    #[serde(default)]
    pub asset_issuer: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StellarAccount {
    pub account_id: String,
    #[serde(deserialize_with = "deserialize_sequence")]
    pub sequence: i64,
    #[serde(default)]
    pub balances: Vec<StellarBalance>,
}

fn deserialize_sequence<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<i64>().map_err(serde::de::Error::custom)
}

/// Alias used by some older call sites for the transaction record type.
pub type HorizonTransactionRecord = HorizonTransaction;

#[derive(Clone)]
pub struct StellarClient {
    horizon: HorizonClient,
    config: StellarConfig,
    http: reqwest::Client,
}

impl StellarClient {
    pub fn new(config: StellarConfig) -> anyhow::Result<Self> {
        let horizon = HorizonClient::new(config.horizon_url.clone())
            .with_timeout(config.request_timeout);
        Ok(Self {
            horizon,
            config,
            http: reqwest::Client::new(),
        })
    }

    pub fn network(&self) -> &str {
        &self.config.network
    }

    pub fn config(&self) -> &StellarConfig {
        &self.config
    }

    /// Lightweight reachability check against Horizon's root endpoint.
    pub async fn health_check(&self) -> anyhow::Result<StellarHealthStatus> {
        let start = Instant::now();
        let url = format!("{}/", self.config.horizon_url.trim_end_matches('/'));
        match self
            .http
            .get(&url)
            .timeout(self.config.request_timeout)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => Ok(StellarHealthStatus {
                is_healthy: true,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: None,
            }),
            Ok(resp) => Ok(StellarHealthStatus {
                is_healthy: false,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: Some(format!("Horizon returned {}", resp.status())),
            }),
            Err(e) => Ok(StellarHealthStatus {
                is_healthy: false,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: Some(e.to_string()),
            }),
        }
    }

    pub async fn account_exists(&self, address: &str) -> anyhow::Result<bool> {
        match self.horizon.get_account_sequence(address).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn get_account(&self, address: &str) -> Result<StellarAccount, StellarError> {
        let url = format!(
            "{}/accounts/{}",
            self.config.horizon_url.trim_end_matches('/'),
            address
        );
        let resp = self
            .http
            .get(&url)
            .timeout(self.config.request_timeout)
            .send()
            .await
            .map_err(|e| StellarError::network_error(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StellarError::AccountNotFound {
                address: address.to_string(),
            });
        }
        if !resp.status().is_success() {
            return Err(StellarError::network_error(format!(
                "Horizon returned {}",
                resp.status()
            )));
        }

        resp.json::<StellarAccount>()
            .await
            .map_err(|e| StellarError::Other(format!("failed to parse account response: {}", e)))
    }

    pub async fn get_transaction_by_hash(
        &self,
        hash: &str,
    ) -> Result<HorizonTransaction, StellarError> {
        match self.horizon.get_transaction(hash).await {
            Ok(Some(tx)) => Ok(tx),
            Ok(None) => Err(StellarError::TransactionFailed {
                reason: "transaction not found".to_string(),
            }),
            Err(e) => Err(StellarError::TransactionFailed {
                reason: e.to_string(),
            }),
        }
    }

    /// Alias — some callers use the `_details` name for the same lookup.
    pub async fn get_transaction_details(
        &self,
        hash: &str,
    ) -> Result<HorizonTransaction, StellarError> {
        self.get_transaction_by_hash(hash).await
    }

    /// Raw operation records for a transaction (Horizon's `/transactions/{hash}/operations`).
    /// Returned as untyped JSON since callers pick out individual fields
    /// (`type`, `to`, `asset_code`, `asset_issuer`, ...) rather than a fixed struct.
    pub async fn get_transaction_operations(
        &self,
        hash: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        #[derive(Deserialize)]
        struct OperationsResponse {
            #[serde(rename = "_embedded")]
            embedded: OperationsEmbedded,
        }
        #[derive(Deserialize)]
        struct OperationsEmbedded {
            records: Vec<serde_json::Value>,
        }

        let url = format!(
            "{}/transactions/{}/operations",
            self.config.horizon_url.trim_end_matches('/'),
            hash
        );
        let resp = self
            .http
            .get(&url)
            .timeout(self.config.request_timeout)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Horizon returned {} for transaction operations", resp.status());
        }

        let parsed: OperationsResponse = resp.json().await?;
        Ok(parsed.embedded.records)
    }

    /// Balance of `asset_code` (optionally scoped to `asset_issuer`) on `address`,
    /// as Horizon's raw decimal string, or `None` if the trustline doesn't exist.
    pub async fn get_asset_balance(
        &self,
        address: &str,
        asset_code: &str,
        asset_issuer: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let account = self
            .get_account(address)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        let balance = account.balances.into_iter().find(|b| {
            let code_matches = b.asset_code.as_deref() == Some(asset_code)
                || (asset_code.eq_ignore_ascii_case("xlm") && b.asset_type == "native");
            let issuer_matches = match asset_issuer {
                Some(issuer) => b.asset_issuer.as_deref() == Some(issuer),
                None => true,
            };
            code_matches && issuer_matches
        });

        Ok(balance.map(|b| b.balance))
    }
}
