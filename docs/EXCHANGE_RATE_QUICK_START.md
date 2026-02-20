# Exchange Rate Service - Quick Start Guide

## 5-Minute Setup

### 1. Add Dependencies

Already included in `Cargo.toml`:
- `bigdecimal` - Decimal arithmetic
- `redis` - Caching
- `sqlx` - Database
- `chrono` - Timestamps

### 2. Initialize Service

```rust
use aframp_backend::services::exchange_rate::{
    ExchangeRateService, ExchangeRateServiceConfig
};
use aframp_backend::services::rate_providers::FixedRateProvider;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use aframp_backend::cache::cache::RedisCache;
use std::sync::Arc;

// Setup
let repo = ExchangeRateRepository::new(pool).with_cache(cache.clone());
let provider = Arc::new(FixedRateProvider::new());

let service = ExchangeRateService::new(repo, ExchangeRateServiceConfig::default())
    .with_cache(cache)
    .add_provider(provider);
```

### 3. Common Operations

#### Get Rate
```rust
let rate = service.get_rate("NGN", "cNGN").await?;
// Returns: 1.0
```

#### Calculate Conversion
```rust
use aframp_backend::services::exchange_rate::{ConversionRequest, ConversionDirection};
use bigdecimal::BigDecimal;

let request = ConversionRequest {
    from_currency: "NGN".to_string(),
    to_currency: "cNGN".to_string(),
    amount: BigDecimal::from(50000),
    direction: ConversionDirection::Buy,
};

let result = service.calculate_conversion(request).await?;
println!("Net amount: {}", result.net_amount);
```

## Common Use Cases

### Onramp Quote (Buy cNGN)

```rust
// User wants to buy cNGN with NGN
let request = ConversionRequest {
    from_currency: "NGN".to_string(),
    to_currency: "cNGN".to_string(),
    amount: BigDecimal::from(100000), // 100,000 NGN
    direction: ConversionDirection::Buy,
};

let quote = service.calculate_conversion(request).await?;

// Response includes:
// - gross_amount: 100,000 cNGN
// - fees.provider_fee: 1,400 cNGN (1.4%)
// - fees.platform_fee: 100 cNGN (0.1%)
// - net_amount: 98,500 cNGN
// - expires_at: timestamp
```

### Offramp Quote (Sell cNGN)

```rust
// User wants to sell cNGN for NGN
let request = ConversionRequest {
    from_currency: "cNGN".to_string(),
    to_currency: "NGN".to_string(),
    amount: BigDecimal::from(50000), // 50,000 cNGN
    direction: ConversionDirection::Sell,
};

let quote = service.calculate_conversion(request).await?;

// Response includes:
// - gross_amount: 50,000 NGN
// - fees.provider_fee: 700 NGN (1.4%)
// - fees.platform_fee: 50 NGN (0.1%)
// - net_amount: 49,250 NGN
// - expires_at: timestamp
```

### Update Rate (Admin)

```rust
let new_rate = BigDecimal::from(1);
service.update_rate("NGN", "cNGN", new_rate, "admin_update").await?;
```

### Get Historical Rate

```rust
let timestamp = chrono::Utc::now() - chrono::Duration::hours(24);
let rate = service.get_historical_rate("NGN", "cNGN", timestamp).await?;
```

## Configuration

### Default Config
```rust
ExchangeRateServiceConfig {
    cache_ttl_seconds: 60,        // 1 minute
    rate_expiry_seconds: 300,     // 5 minutes
    enable_validation: true,
    max_rate_deviation: 0.0001,   // ±0.01% for cNGN
}
```

### Custom Config
```rust
use std::str::FromStr;

let config = ExchangeRateServiceConfig {
    cache_ttl_seconds: 120,       // 2 minutes
    rate_expiry_seconds: 600,     // 10 minutes
    enable_validation: true,
    max_rate_deviation: BigDecimal::from_str("0.001").unwrap(),
};
```

## Error Handling

```rust
match service.get_rate("NGN", "cNGN").await {
    Ok(rate) => println!("Rate: {}", rate),
    Err(ExchangeRateError::RateNotFound { from, to }) => {
        eprintln!("Rate not found: {} -> {}", from, to);
    }
    Err(ExchangeRateError::InvalidRate(msg)) => {
        eprintln!("Invalid rate: {}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Testing

### Unit Test
```rust
#[tokio::test]
async fn test_get_rate() {
    let service = setup_service().await;
    let rate = service.get_rate("NGN", "cNGN").await.unwrap();
    assert_eq!(rate, BigDecimal::from(1));
}
```

### Integration Test
```bash
cargo test --test exchange_rate_service_test --features database,cache
```

## Performance Tips

1. **Enable caching**: 95%+ cache hit rate
2. **Connection pooling**: Use appropriate pool sizes
3. **Batch operations**: Group rate updates
4. **Monitor metrics**: Track cache hit rate

## Troubleshooting

### Rate not found
- Check if provider is configured
- Verify currency pair is supported
- Check database for stored rates

### Cache miss
- Verify Redis is running
- Check cache configuration
- Monitor cache hit rate

### Invalid rate
- Check rate validation rules
- Verify rate is positive
- For cNGN, ensure rate is ~1.0

## Next Steps

- Read [full documentation](EXCHANGE_RATE_SERVICE.md)
- Check [examples](../examples/exchange_rate_service_example.rs)
- Review [API reference](https://docs.rs/aframp-backend)

## Support

- GitHub Issues: Report bugs or request features
- Documentation: Comprehensive guides in `docs/`
- Examples: Working code in `examples/`
