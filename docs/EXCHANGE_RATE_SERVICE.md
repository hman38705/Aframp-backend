# Exchange Rate Service Documentation

## Overview

The Exchange Rate Service manages exchange rates between currencies with caching, fee calculation, and historical rate storage. It's designed to support the cNGN 1:1 peg with NGN while being extensible for future multi-currency support.

## Features

- ✅ Fixed 1:1 rate for cNGN/NGN peg
- ✅ Redis caching for fast rate lookups (< 2ms)
- ✅ Conversion calculations with fee integration
- ✅ Historical rate storage for audit trail
- ✅ Rate validation and monitoring
- ✅ Extensible rate provider system
- ✅ Support for future external API integration

## Architecture

### Components

1. **ExchangeRateService**: Main service coordinating rate operations
2. **RateProvider**: Trait for implementing rate sources
3. **ExchangeRateRepository**: Database layer for rate persistence
4. **RedisCache**: Caching layer for performance
5. **FeeStructureService**: Integration for fee calculations

### Rate Providers

#### FixedRateProvider
Returns fixed 1:1 rate for cNGN/NGN peg. Always healthy, no external dependencies.

```rust
let provider = Arc::new(FixedRateProvider::new());
```

#### ExternalApiProvider (Future)
Placeholder for external API integration (CoinGecko, Fixer.io, etc.)

```rust
let provider = ExternalApiProvider::new(
    "https://api.coingecko.com".to_string(),
    Some("api_key".to_string())
)
.with_timeout(10)
.add_supported_pair("USD".to_string(), "NGN".to_string());
```

#### AggregatedRateProvider
Combines multiple providers with aggregation strategies (average, median, first).

```rust
let provider = AggregatedRateProvider::new(AggregationStrategy::Median)
    .add_provider(Box::new(provider1))
    .add_provider(Box::new(provider2));
```

## Usage

### Basic Setup

```rust
use aframp_backend::services::exchange_rate::{
    ExchangeRateService, ExchangeRateServiceConfig
};
use aframp_backend::services::rate_providers::FixedRateProvider;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use std::sync::Arc;

// Create repository
let repo = ExchangeRateRepository::new(pool.clone())
    .with_cache(cache.clone());

// Create rate provider
let provider = Arc::new(FixedRateProvider::new());

// Create service
let service = ExchangeRateService::new(
    repo,
    ExchangeRateServiceConfig::default()
)
.with_cache(cache)
.add_provider(provider);
```

### Get Exchange Rate

```rust
// Get current rate
let rate = service.get_rate("NGN", "cNGN").await?;
println!("Rate: {}", rate); // Output: 1
```

### Calculate Conversion with Fees

```rust
use aframp_backend::services::exchange_rate::{
    ConversionRequest, ConversionDirection
};
use bigdecimal::BigDecimal;

// Onramp: NGN -> cNGN
let request = ConversionRequest {
    from_currency: "NGN".to_string(),
    to_currency: "cNGN".to_string(),
    amount: BigDecimal::from(50000),
    direction: ConversionDirection::Buy,
};

let result = service.calculate_conversion(request).await?;

println!("User pays: {} NGN", result.from_amount);
println!("Gross amount: {} cNGN", result.gross_amount);
println!("Provider fee: {} cNGN", result.fees.provider_fee);
println!("Platform fee: {} cNGN", result.fees.platform_fee);
println!("Total fees: {} cNGN", result.fees.total_fees);
println!("User receives: {} cNGN", result.net_amount);
```

### Update Exchange Rate

```rust
use bigdecimal::BigDecimal;

let new_rate = BigDecimal::from(1);
service.update_rate(
    "NGN",
    "cNGN",
    new_rate,
    "manual_update"
).await?;
```

### Get Historical Rate

```rust
use chrono::Utc;

let timestamp = Utc::now() - chrono::Duration::hours(1);
let historical_rate = service.get_historical_rate(
    "NGN",
    "cNGN",
    timestamp
).await?;
```

### Invalidate Cache

```rust
// Manually invalidate cached rate
service.invalidate_cache("NGN", "cNGN").await?;
```

## Configuration

### ExchangeRateServiceConfig

```rust
use aframp_backend::services::exchange_rate::ExchangeRateServiceConfig;
use bigdecimal::BigDecimal;
use std::str::FromStr;

let config = ExchangeRateServiceConfig {
    cache_ttl_seconds: 60,           // Cache TTL for rates
    rate_expiry_seconds: 300,        // Quote expiry time
    enable_validation: true,         // Enable rate validation
    max_rate_deviation: BigDecimal::from_str("0.0001").unwrap(), // Max deviation for cNGN
};
```

## Caching Strategy

### Cache Keys

Format: `v1:rate:{from_currency}:{to_currency}`

Examples:
- `v1:rate:NGN:cNGN`
- `v1:rate:USD:NGN`

### TTL Configuration

- **cNGN rates**: 60 seconds (fixed peg, rarely changes)
- **External rates**: 300 seconds (5 minutes)

### Cache Flow

1. Check Redis for rate key
2. If cache hit: Return cached rate (< 2ms)
3. If cache miss: Fetch from provider
4. Store in Redis with TTL
5. Return fresh rate

### Cache Invalidation

- **Manual**: When rate updated by admin
- **Automatic**: TTL expiry
- **Bulk**: Clear all rates on system update

## Fee Integration

The service integrates with the Fee Structure Service to calculate fees:

### Fee Types

1. **Provider Fee**: 1.4% (140 bps)
2. **Platform Fee**: 0.1% (10 bps)

### Fee Calculation Flow

```
1. Get exchange rate (1.0 for NGN/cNGN)
2. Calculate gross amount (amount × rate)
3. Fetch applicable fees from fee service
4. Apply fees to gross amount
5. Return net amount with breakdown
```

### Example Calculation

```
User pays: 50,000 NGN
Base conversion: 50,000 NGN × 1.0 = 50,000 cNGN
Provider fee: 1.4% = 700 cNGN
Platform fee: 0.1% = 50 cNGN
Total fees: 750 cNGN
User receives: 49,250 cNGN
```

## Rate Validation

### Validation Rules

#### For cNGN/NGN:
- Rate must be 1.0 (±0.0001 tolerance)
- Deviation triggers immediate alert
- Failsafe: Use 1.0 if source returns invalid data

#### For External Rates:
- Rate must be within expected range
- Compare across multiple sources
- Reject rates differing > 5% from previous
- Alert on significant changes (> 10% in 1 hour)

### Validation Implementation

```rust
// Automatic validation on rate update
service.update_rate("NGN", "cNGN", rate, "source").await?;
// Returns error if rate is invalid
```

## Historical Rate Storage

### Purpose

- Audit trail for conversions
- Dispute resolution
- Compliance requirements
- Rate trend analysis

### Database Schema

```sql
CREATE TABLE exchange_rates (
  id TEXT PRIMARY KEY,
  from_currency TEXT NOT NULL,
  to_currency TEXT NOT NULL,
  rate TEXT NOT NULL,
  source TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (from_currency, to_currency)
);
```

### Retention Policy

- Keep all rates for at least 7 years (compliance)
- Archive old rates to separate table (> 1 year old)
- Index on currency_pair and created_at for queries

## Error Handling

### Error Types

```rust
pub enum ExchangeRateError {
    Database(DatabaseError),
    RateNotFound { from: String, to: String },
    InvalidRate(String),
    ProviderError(String),
    FeeCalculationError(String),
    InvalidAmount(String),
}
```

### Error Scenarios

#### Rate Fetch Failures
- Return cached rate if available (stale is better than none)
- Log error and alert monitoring
- Use fallback rate provider
- Return error if all sources fail

#### Cache Failures
- Continue without cache (slower but functional)
- Log cache unavailability
- Alert if Redis down > 5 minutes

#### Invalid Rates
- Reject rates outside expected range
- Use last known good rate
- Alert immediately
- Manual intervention required

## Performance

### Metrics

- **Cache hit**: < 2ms
- **Cache miss**: < 500ms (including external API call)
- **Cache hit rate**: > 95%
- **Conversion calculation**: < 10ms

### Optimization Tips

1. **Enable caching**: Always use Redis for production
2. **Provider health checks**: Monitor provider availability
3. **Connection pooling**: Use appropriate pool sizes
4. **Rate limiting**: Implement for external APIs

## Monitoring

### Key Metrics

- Rate fetch success rate
- Rate cache hit rate
- Average rate staleness
- Conversion calculation count
- Fee application errors

### Alerts

- Rate not updated in 5 minutes (for external sources)
- Rate deviates from expected (> 1% for cNGN)
- Cache unavailable (> 1 minute)
- High rate fetch failure rate (> 5%)

## Testing

### Unit Tests

```bash
cargo test --lib services::exchange_rate
```

### Integration Tests

```bash
# Requires database and Redis
cargo test --test exchange_rate_service_test --features database,cache
```

### Example Usage

```bash
cargo run --example exchange_rate_service_example --features database,cache
```

## Future Enhancements

### Multi-Currency Support

When adding USD/NGN:

```rust
// Rate triangulation
// User wants: cNGN → USD
// Rates available: cNGN/NGN (1.0), USD/NGN (1500.0)
// Calculated rate: 1 cNGN = 0.000667 USD
```

### Multi-Source Aggregation

```rust
// Fetch from 3 sources
// - CoinGecko: 1498.00 NGN/USD
// - Fixer.io: 1502.00 NGN/USD
// - CBN: 1500.00 NGN/USD
// Average: 1500.00 NGN/USD (median preferred)
```

### Rate Prediction

- Machine learning for rate forecasting
- Trend analysis
- Volatility indicators

## Best Practices

1. **Always use BigDecimal**: Never use float for monetary calculations
2. **Cache aggressively**: cNGN rate rarely changes
3. **Store rates with transactions**: For dispute resolution
4. **Document fee calculations**: For transparency
5. **Monitor rate staleness**: Alert on outdated rates
6. **Test rounding behavior**: At various amounts
7. **Validate input amounts**: Prevent negative/zero amounts

## API Reference

See the [API documentation](https://docs.rs/aframp-backend) for detailed API reference.

## Support

For issues or questions:
- GitHub Issues: [aframp-backend/issues](https://github.com/yourusername/aframp-backend/issues)
- Documentation: [docs/](../docs/)
- Examples: [examples/](../examples/)

## License

MIT - see LICENSE file
