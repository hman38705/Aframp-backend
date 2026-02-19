# Unified Blockchain Service Interface

## Overview

The Unified Blockchain Service Interface provides a common abstraction layer for interacting with different blockchain networks. This allows the application to work with multiple chains through a consistent API, making it easier to add support for new blockchains and aggregate data across chains.

## Architecture

### Core Components

1. **BlockchainService Trait** (`src/chains/traits.rs`)
   - Common interface for all blockchain operations
   - Chain-agnostic methods for accounts, balances, and transactions
   - Async/await support via `async-trait`

2. **StellarBlockchainService** (`src/chains/stellar/service.rs`)
   - Stellar-specific implementation of BlockchainService
   - Wraps the existing StellarClient
   - Converts Stellar-specific types to common types

3. **MultiChainBalanceAggregator** (`src/chains/traits.rs`)
   - Aggregates balances across multiple chains
   - Provides unified view of assets
   - Health monitoring for all chains

## Key Features

### Chain-Agnostic Operations

```rust
use aframp_backend::chains::traits::BlockchainService;

// Works with any blockchain implementation
async fn check_balance(service: &dyn BlockchainService, address: &str) {
    match service.get_balances(address).await {
        Ok(balances) => {
            for balance in balances {
                println!("{}: {}", balance.asset_code, balance.balance);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Multi-Chain Balance Aggregation

```rust
use aframp_backend::chains::traits::MultiChainBalanceAggregator;

let chains: Vec<Box<dyn BlockchainService>> = vec![
    Box::new(stellar_service),
    // Add more chains here
];

let aggregator = MultiChainBalanceAggregator::new(chains);

// Get balances across all chains
let all_balances = aggregator.get_all_balances(address).await;

// Get total for specific asset
let total = aggregator.get_asset_total(address, "USDC").await?;
```

### Health Monitoring

```rust
// Check health of a single chain
let health = service.health_check().await?;
println!("Chain {} is {}", 
    health.chain_id, 
    if health.is_healthy { "healthy" } else { "unhealthy" }
);

// Check health of all chains
let health_results = aggregator.health_check_all().await;
for (chain_id, health) in health_results {
    println!("{}: {}ms", chain_id, health.response_time_ms);
}
```

## BlockchainService Trait

### Methods

#### `chain_id() -> &str`
Returns the chain identifier (e.g., "stellar", "ethereum")

#### `account_exists(address: &str) -> BlockchainResult<bool>`
Checks if an account exists on the blockchain

#### `get_account(address: &str) -> BlockchainResult<AccountInfo>`
Gets account information including balances and metadata

#### `get_balances(address: &str) -> BlockchainResult<Vec<AssetBalance>>`
Gets all asset balances for an account

#### `get_asset_balance(address: &str, asset_code: &str, issuer: Option<&str>) -> BlockchainResult<Option<String>>`
Gets balance for a specific asset

#### `submit_transaction(signed_tx: &str) -> BlockchainResult<TransactionResult>`
Submits a signed transaction to the blockchain

#### `get_transaction(tx_hash: &str) -> BlockchainResult<TransactionResult>`
Gets transaction details by hash

#### `health_check() -> BlockchainResult<ChainHealthStatus>`
Performs health check on blockchain connection

#### `validate_address(address: &str) -> BlockchainResult<()>`
Validates an address format

## Common Types

### AccountInfo
```rust
pub struct AccountInfo {
    pub address: String,
    pub sequence: String,
    pub balances: Vec<AssetBalance>,
    pub metadata: HashMap<String, String>,
}
```

### AssetBalance
```rust
pub struct AssetBalance {
    pub asset_code: String,
    pub issuer: Option<String>,
    pub balance: String,
    pub asset_type: String,
    pub limit: Option<String>,
}
```

### TransactionResult
```rust
pub struct TransactionResult {
    pub hash: String,
    pub successful: bool,
    pub ledger: Option<i64>,
    pub fee_charged: Option<String>,
    pub raw_response: serde_json::Value,
}
```

### ChainHealthStatus
```rust
pub struct ChainHealthStatus {
    pub is_healthy: bool,
    pub chain_id: String,
    pub response_time_ms: u64,
    pub last_check: String,
    pub error_message: Option<String>,
}
```

## Error Handling

### BlockchainError
Unified error type for all blockchain operations:

- `AccountNotFound` - Account doesn't exist
- `InvalidAddress` - Invalid address format
- `NetworkError` - Network communication error
- `TransactionFailed` - Transaction submission failed
- `Timeout` - Operation timed out
- `RateLimitExceeded` - Rate limit hit
- `InsufficientBalance` - Not enough balance
- `AssetNotFound` - Asset not found or not trusted
- `ConfigError` - Configuration error
- `SerializationError` - Serialization/deserialization error
- `Other` - Generic error

All errors implement `std::error::Error` and can be converted to/from chain-specific errors.

## Usage Examples

### Basic Account Query

```rust
use aframp_backend::chains::stellar::client::StellarClient;
use aframp_backend::chains::stellar::config::StellarConfig;
use aframp_backend::chains::stellar::service::StellarBlockchainService;
use aframp_backend::chains::traits::BlockchainService;

let config = StellarConfig::from_env()?;
let client = StellarClient::new(config)?;
let service = StellarBlockchainService::new(client);

let address = "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX";

if service.account_exists(address).await? {
    let account = service.get_account(address).await?;
    println!("Account has {} balances", account.balances.len());
}
```

### Multi-Chain Aggregation

```rust
use aframp_backend::chains::traits::MultiChainBalanceAggregator;

let stellar_service = StellarBlockchainService::new(stellar_client);
// Add more chains as they're implemented

let chains: Vec<Box<dyn BlockchainService>> = vec![
    Box::new(stellar_service),
];

let aggregator = MultiChainBalanceAggregator::new(chains);

// Get all balances across all chains
let all_balances = aggregator.get_all_balances(address).await;

for (chain_id, result) in all_balances {
    match result {
        Ok(balances) => {
            println!("{} chain:", chain_id);
            for balance in balances {
                println!("  {}: {}", balance.asset_code, balance.balance);
            }
        }
        Err(e) => eprintln!("{} error: {}", chain_id, e),
    }
}
```

### Transaction Submission

```rust
// Submit a signed transaction
let result = service.submit_transaction(signed_xdr).await?;

if result.successful {
    println!("Transaction successful: {}", result.hash);
    println!("Included in ledger: {:?}", result.ledger);
} else {
    println!("Transaction failed");
}
```

## Adding New Blockchain Support

To add support for a new blockchain:

1. **Create a client module** (e.g., `src/chains/ethereum/client.rs`)
   - Implement chain-specific API calls
   - Handle chain-specific types and errors

2. **Create a service wrapper** (e.g., `src/chains/ethereum/service.rs`)
   - Implement the `BlockchainService` trait
   - Convert chain-specific types to common types
   - Convert chain-specific errors to `BlockchainError`

3. **Update module exports** (`src/chains/mod.rs`)
   ```rust
   pub mod ethereum;
   pub mod stellar;
   pub mod traits;
   ```

4. **Use in aggregator**
   ```rust
   let chains: Vec<Box<dyn BlockchainService>> = vec![
       Box::new(stellar_service),
       Box::new(ethereum_service),
   ];
   ```

## Testing

Run the demo example:
```bash
cargo run --example blockchain_service_demo
```

Run unit tests:
```bash
cargo test --lib chains::stellar::service
```

## Benefits

1. **Consistency** - Same API for all blockchains
2. **Flexibility** - Easy to add new chains
3. **Aggregation** - Unified view across chains
4. **Type Safety** - Compile-time guarantees
5. **Testability** - Easy to mock for testing
6. **Maintainability** - Changes in one place

## Future Enhancements

- [ ] Add Ethereum/EVM support
- [ ] Add Bitcoin support
- [ ] Implement caching layer
- [ ] Add transaction building helpers
- [ ] Support for batch operations
- [ ] WebSocket support for real-time updates
- [ ] Gas/fee estimation across chains
- [ ] Cross-chain asset tracking

## Related Documentation

- [Stellar Integration](../STELLAR_INTEGRATION.md)
- [Caching Strategy](./CACHING.md)
- [API Documentation](../README.md)
