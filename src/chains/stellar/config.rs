//! Configuration for the [`super::client::StellarClient`] compatibility shim.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct StellarConfig {
    pub network: String,
    pub horizon_url: String,
    pub request_timeout: Duration,
    pub max_retries: u32,
}

impl StellarConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let network = std::env::var("STELLAR_NETWORK").unwrap_or_else(|_| "testnet".to_string());
        let horizon_url = std::env::var("STELLAR_HORIZON_URL").unwrap_or_else(|_| {
            if network == "mainnet" || network == "public" {
                "https://horizon.stellar.org".to_string()
            } else {
                "https://horizon-testnet.stellar.org".to_string()
            }
        });
        let request_timeout = Duration::from_secs(
            std::env::var("STELLAR_REQUEST_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15),
        );
        let max_retries = std::env::var("STELLAR_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        Ok(Self {
            network,
            horizon_url,
            request_timeout,
            max_retries,
        })
    }
}

impl Default for StellarConfig {
    fn default() -> Self {
        Self {
            network: "testnet".to_string(),
            horizon_url: "https://horizon-testnet.stellar.org".to_string(),
            request_timeout: Duration::from_secs(15),
            max_retries: 3,
        }
    }
}
