use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Common result type for blockchain operations
pub type BlockchainResult<T> = Result<T, BlockchainError>;

/// Unified error type for all blockchain operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockchainError {
    /// Account not found on the blockchain
    AccountNotFound { address: String },
    /// Invalid address format
    InvalidAddress { address: String },
    /// Network communication error
    NetworkError { message: String },
    /// Transaction submission failed
    TransactionFailed { message: String },
    /// Operation timed out
    Timeout { seconds: u64 },
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Insufficient balance for operation
    InsufficientBalance { required: String, available: String },
    /// Asset not found or not trusted
    AssetNotFound { asset_code: String },
    /// Configuration error
    ConfigError { message: String },
    /// Serialization/deserialization error
    SerializationError { message: String },
    /// Generic error
    Other { message: String },
}

impl std::fmt::Display for BlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockchainError::AccountNotFound { address } => {
                write!(f, "Account not found: {}", address)
            }
            BlockchainError::InvalidAddress { address } => {
                write!(f, "Invalid address: {}", address)
            }
            BlockchainError::NetworkError { message } => write!(f, "Network error: {}", message),
            BlockchainError::TransactionFailed { message } => {
                write!(f, "Transaction failed: {}", message)
            }
            BlockchainError::Timeout { seconds } => {
                write!(f, "Operation timed out after {} seconds", seconds)
            }
            BlockchainError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            BlockchainError::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient balance: required {}, available {}",
                    required, available
                )
            }
            BlockchainError::AssetNotFound { asset_code } => {
                write!(f, "Asset not found: {}", asset_code)
            }
            BlockchainError::ConfigError { message } => write!(f, "Config error: {}", message),
            BlockchainError::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
            BlockchainError::Other { message } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for BlockchainError {}

/// Represents a balance for a specific asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBalance {
    /// Asset code (e.g., "XLM", "cNGN", "AFRI")
    pub asset_code: String,
    /// Asset issuer address (None for native assets)
    pub issuer: Option<String>,
    /// Balance amount as string
    pub balance: String,
    /// Asset type (e.g., "native", "credit_alphanum4")
    pub asset_type: String,
    /// Optional limit for trustline
    pub limit: Option<String>,
}

/// Represents account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Account address
    pub address: String,
    /// Account sequence number (for transaction ordering)
    pub sequence: String,
    /// List of asset balances
    pub balances: Vec<AssetBalance>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Health status for blockchain connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainHealthStatus {
    /// Whether the chain is healthy
    pub is_healthy: bool,
    /// Chain identifier (e.g., "stellar", "ethereum")
    pub chain_id: String,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Last check timestamp
    pub last_check: String,
    /// Error message if unhealthy
    pub error_message: Option<String>,
}

/// Transaction submission result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// Transaction hash
    pub hash: String,
    /// Whether transaction was successful
    pub successful: bool,
    /// Ledger number where transaction was included
    pub ledger: Option<i64>,
    /// Transaction fee charged
    pub fee_charged: Option<String>,
    /// Raw response from blockchain
    pub raw_response: serde_json::Value,
}

/// Unified blockchain service trait
///
/// This trait provides a common interface for interacting with different blockchains.
/// Implementations should handle chain-specific details internally.
#[async_trait]
pub trait BlockchainService: Send + Sync {
    /// Get the chain identifier (e.g., "stellar", "ethereum")
    fn chain_id(&self) -> &str;

    /// Check if an account exists on the blockchain
    async fn account_exists(&self, address: &str) -> BlockchainResult<bool>;

    /// Get account information including balances
    async fn get_account(&self, address: &str) -> BlockchainResult<AccountInfo>;

    /// Get all balances for an account
    async fn get_balances(&self, address: &str) -> BlockchainResult<Vec<AssetBalance>>;

    /// Get balance for a specific asset
    async fn get_asset_balance(
        &self,
        address: &str,
        asset_code: &str,
        issuer: Option<&str>,
    ) -> BlockchainResult<Option<String>>;

    /// Submit a signed transaction to the blockchain
    async fn submit_transaction(&self, signed_tx: &str) -> BlockchainResult<TransactionResult>;

    /// Get transaction details by hash
    async fn get_transaction(&self, tx_hash: &str) -> BlockchainResult<TransactionResult>;

    /// Perform health check on blockchain connection
    async fn health_check(&self) -> BlockchainResult<ChainHealthStatus>;

    /// Validate an address format
    fn validate_address(&self, address: &str) -> BlockchainResult<()>;
}

/// Multi-chain balance aggregator
///
/// Aggregates balances across multiple blockchain services
pub struct MultiChainBalanceAggregator {
    chains: Vec<Box<dyn BlockchainService>>,
}

impl MultiChainBalanceAggregator {
    /// Create a new aggregator with the given blockchain services
    pub fn new(chains: Vec<Box<dyn BlockchainService>>) -> Self {
        Self { chains }
    }

    /// Get balances across all chains for an address
    pub async fn get_all_balances(
        &self,
        address: &str,
    ) -> HashMap<String, BlockchainResult<Vec<AssetBalance>>> {
        let mut results = HashMap::new();

        for chain in &self.chains {
            let chain_id = chain.chain_id().to_string();
            let balances = chain.get_balances(address).await;
            results.insert(chain_id, balances);
        }

        results
    }

    /// Get total balance for a specific asset across all chains
    pub async fn get_asset_total(
        &self,
        address: &str,
        asset_code: &str,
    ) -> BlockchainResult<String> {
        let mut total = 0.0;

        for chain in &self.chains {
            if let Ok(Some(balance)) = chain.get_asset_balance(address, asset_code, None).await {
                if let Ok(amount) = balance.parse::<f64>() {
                    total += amount;
                }
            }
        }

        Ok(total.to_string())
    }

    /// Check health of all chains
    pub async fn health_check_all(&self) -> HashMap<String, ChainHealthStatus> {
        let mut results = HashMap::new();

        for chain in &self.chains {
            let chain_id = chain.chain_id().to_string();
            if let Ok(status) = chain.health_check().await {
                results.insert(chain_id, status);
            }
        }

        results
    }
}
