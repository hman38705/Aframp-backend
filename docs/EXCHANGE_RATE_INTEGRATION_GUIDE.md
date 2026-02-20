# Exchange Rate Service - Integration Guide

## Overview

This guide shows how to integrate the Exchange Rate Service into your Aframp backend application for onramp/offramp quote calculations.

## Prerequisites

- PostgreSQL database with `exchange_rates` table (already exists)
- Redis instance for caching
- Fee structures configured in database

## Step 1: Application State Setup

Add the exchange rate service to your application state:

```rust
// src/main.rs or src/lib.rs

use aframp_backend::services::exchange_rate::{
    ExchangeRateService, ExchangeRateServiceConfig
};
use aframp_backend::services::rate_providers::FixedRateProvider;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use aframp_backend::database::fee_structure_repository::FeeStructureRepository;
use aframp_backend::services::fee_structure::FeeStructureService;
use aframp_backend::cache::cache::RedisCache;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub cache: RedisCache,
    pub exchange_rate_service: Arc<ExchangeRateService>,
    // ... other services
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize database pool
        let db_pool = PgPool::connect(&config.database.url).await?;
        
        // Initialize Redis cache
        let cache_pool = init_cache_pool(config.cache).await?;
        let cache = RedisCache::new(cache_pool);
        
        // Create repositories
        let rate_repo = ExchangeRateRepository::new(db_pool.clone())
            .with_cache(cache.clone());
        let fee_repo = FeeStructureRepository::new(db_pool.clone());
        
        // Create services
        let fee_service = Arc::new(FeeStructureService::new(fee_repo));
        let rate_provider = Arc::new(FixedRateProvider::new());
        
        let exchange_rate_service = Arc::new(
            ExchangeRateService::new(
                rate_repo,
                ExchangeRateServiceConfig::default()
            )
            .with_cache(cache.clone())
            .add_provider(rate_provider)
            .with_fee_service(fee_service)
        );
        
        Ok(Self {
            db_pool,
            cache,
            exchange_rate_service,
        })
    }
}
```

## Step 2: Onramp Quote Endpoint

Create or update your onramp quote endpoint:

```rust
// src/api/onramp.rs

use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use bigdecimal::BigDecimal;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub struct OnrampQuoteRequest {
    pub from_currency: String,  // e.g., "NGN"
    pub to_currency: String,    // e.g., "cNGN"
    pub amount: String,         // e.g., "50000"
}

#[derive(Debug, Serialize)]
pub struct OnrampQuoteResponse {
    pub from_currency: String,
    pub to_currency: String,
    pub from_amount: String,
    pub exchange_rate: String,
    pub gross_amount: String,
    pub fees: FeeBreakdown,
    pub net_amount: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct FeeBreakdown {
    pub provider_fee: String,
    pub platform_fee: String,
    pub total_fees: String,
}

pub async fn get_onramp_quote(
    State(state): State<AppState>,
    Json(request): Json<OnrampQuoteRequest>,
) -> Result<Json<OnrampQuoteResponse>, (StatusCode, String)> {
    // Parse amount
    let amount = BigDecimal::from_str(&request.amount)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid amount".to_string()))?;
    
    // Validate amount
    if amount <= BigDecimal::from(0) {
        return Err((StatusCode::BAD_REQUEST, "Amount must be positive".to_string()));
    }
    
    // Create conversion request
    let conversion_request = ConversionRequest {
        from_currency: request.from_currency.clone(),
        to_currency: request.to_currency.clone(),
        amount,
        direction: ConversionDirection::Buy,
    };
    
    // Calculate conversion
    let result = state.exchange_rate_service
        .calculate_conversion(conversion_request)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Build response
    let response = OnrampQuoteResponse {
        from_currency: result.from_currency,
        to_currency: result.to_currency,
        from_amount: result.from_amount,
        exchange_rate: result.base_rate,
        gross_amount: result.gross_amount,
        fees: FeeBreakdown {
            provider_fee: result.fees.provider_fee,
            platform_fee: result.fees.platform_fee,
            total_fees: result.fees.total_fees,
        },
        net_amount: result.net_amount,
        expires_at: result.expires_at.to_rfc3339(),
    };
    
    Ok(Json(response))
}
```

## Step 3: Offramp Quote Endpoint

Create or update your offramp quote endpoint:

```rust
// src/api/offramp.rs

#[derive(Debug, Deserialize)]
pub struct OfframpQuoteRequest {
    pub from_currency: String,  // e.g., "cNGN"
    pub to_currency: String,    // e.g., "NGN"
    pub amount: String,         // e.g., "50000"
}

pub async fn get_offramp_quote(
    State(state): State<AppState>,
    Json(request): Json<OfframpQuoteRequest>,
) -> Result<Json<OnrampQuoteResponse>, (StatusCode, String)> {
    // Parse amount
    let amount = BigDecimal::from_str(&request.amount)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid amount".to_string()))?;
    
    // Validate amount
    if amount <= BigDecimal::from(0) {
        return Err((StatusCode::BAD_REQUEST, "Amount must be positive".to_string()));
    }
    
    // Create conversion request
    let conversion_request = ConversionRequest {
        from_currency: request.from_currency.clone(),
        to_currency: request.to_currency.clone(),
        amount,
        direction: ConversionDirection::Sell,
    };
    
    // Calculate conversion
    let result = state.exchange_rate_service
        .calculate_conversion(conversion_request)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Build response (same structure as onramp)
    let response = OnrampQuoteResponse {
        from_currency: result.from_currency,
        to_currency: result.to_currency,
        from_amount: result.from_amount,
        exchange_rate: result.base_rate,
        gross_amount: result.gross_amount,
        fees: FeeBreakdown {
            provider_fee: result.fees.provider_fee,
            platform_fee: result.fees.platform_fee,
            total_fees: result.fees.total_fees,
        },
        net_amount: result.net_amount,
        expires_at: result.expires_at.to_rfc3339(),
    };
    
    Ok(Json(response))
}
```

## Step 4: Router Configuration

Add the routes to your Axum router:

```rust
// src/main.rs

use axum::{
    routing::{get, post},
    Router,
};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Onramp routes
        .route("/api/onramp/quote", post(onramp::get_onramp_quote))
        
        // Offramp routes
        .route("/api/offramp/quote", post(offramp::get_offramp_quote))
        
        // Admin routes (optional)
        .route("/api/admin/rates", get(admin::get_rates))
        .route("/api/admin/rates", post(admin::update_rate))
        
        .with_state(state)
}
```

## Step 5: Admin Endpoints (Optional)

Create admin endpoints for rate management:

```rust
// src/api/admin.rs

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct UpdateRateRequest {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct RateResponse {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: String,
    pub source: String,
    pub updated_at: String,
}

pub async fn update_rate(
    State(state): State<AppState>,
    Json(request): Json<UpdateRateRequest>,
) -> Result<Json<RateResponse>, (StatusCode, String)> {
    let rate = BigDecimal::from_str(&request.rate)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid rate".to_string()))?;
    
    state.exchange_rate_service
        .update_rate(
            &request.from_currency,
            &request.to_currency,
            rate.clone(),
            &request.source,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(RateResponse {
        from_currency: request.from_currency,
        to_currency: request.to_currency,
        rate: rate.to_string(),
        source: request.source,
        updated_at: chrono::Utc::now().to_rfc3339(),
    }))
}

pub async fn get_rates(
    State(state): State<AppState>,
) -> Result<Json<Vec<RateResponse>>, (StatusCode, String)> {
    // Get all rates from repository
    let rates = state.exchange_rate_service
        .repository
        .find_all()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let response: Vec<RateResponse> = rates
        .into_iter()
        .map(|r| RateResponse {
            from_currency: r.from_currency,
            to_currency: r.to_currency,
            rate: r.rate,
            source: r.source.unwrap_or_default(),
            updated_at: r.updated_at.to_rfc3339(),
        })
        .collect();
    
    Ok(Json(response))
}
```

## Step 6: Transaction Recording

When processing actual transactions, store the rate used:

```rust
// In your transaction processing logic

pub async fn process_onramp_transaction(
    state: &AppState,
    transaction_id: &str,
    from_currency: &str,
    to_currency: &str,
    amount: BigDecimal,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the rate at transaction time
    let rate = state.exchange_rate_service
        .get_rate(from_currency, to_currency)
        .await?;
    
    // Store transaction with rate
    let transaction = Transaction {
        id: transaction_id.to_string(),
        from_currency: from_currency.to_string(),
        to_currency: to_currency.to_string(),
        amount: amount.to_string(),
        exchange_rate: rate.to_string(),
        timestamp: Utc::now(),
        // ... other fields
    };
    
    state.transaction_repo.insert(&transaction).await?;
    
    Ok(())
}
```

## Step 7: Environment Configuration

Update your `.env` file:

```bash
# Database
DATABASE_URL=postgresql://user:password@localhost/aframp
DB_MAX_CONNECTIONS=20

# Redis
REDIS_URL=redis://localhost:6379
CACHE_MAX_CONNECTIONS=10

# Exchange Rate Service (optional overrides)
EXCHANGE_RATE_CACHE_TTL=60
EXCHANGE_RATE_QUOTE_EXPIRY=300
EXCHANGE_RATE_ENABLE_VALIDATION=true
```

## Step 8: Monitoring Setup

Add monitoring for the exchange rate service:

```rust
// src/middleware/metrics.rs

use prometheus::{
    register_histogram_vec, register_counter_vec,
    HistogramVec, CounterVec,
};

lazy_static! {
    pub static ref RATE_FETCH_DURATION: HistogramVec = register_histogram_vec!(
        "exchange_rate_fetch_duration_seconds",
        "Time to fetch exchange rate",
        &["from_currency", "to_currency", "cache_hit"]
    ).unwrap();
    
    pub static ref CONVERSION_CALCULATIONS: CounterVec = register_counter_vec!(
        "exchange_rate_conversions_total",
        "Total conversion calculations",
        &["from_currency", "to_currency", "direction"]
    ).unwrap();
    
    pub static ref RATE_VALIDATION_FAILURES: CounterVec = register_counter_vec!(
        "exchange_rate_validation_failures_total",
        "Total rate validation failures",
        &["from_currency", "to_currency", "reason"]
    ).unwrap();
}
```

## Step 9: Testing

Test the integration:

```bash
# Start services
docker-compose up -d postgres redis

# Run migrations
sqlx migrate run

# Start the server
cargo run

# Test onramp quote
curl -X POST http://localhost:8000/api/onramp/quote \
  -H "Content-Type: application/json" \
  -d '{
    "from_currency": "NGN",
    "to_currency": "cNGN",
    "amount": "50000"
  }'

# Expected response:
# {
#   "from_currency": "NGN",
#   "to_currency": "cNGN",
#   "from_amount": "50000",
#   "exchange_rate": "1",
#   "gross_amount": "50000",
#   "fees": {
#     "provider_fee": "700",
#     "platform_fee": "50",
#     "total_fees": "750"
#   },
#   "net_amount": "49250",
#   "expires_at": "2026-02-20T10:35:00Z"
# }
```

## Step 10: Production Deployment

### Pre-deployment Checklist

- [ ] Database migrations applied
- [ ] Redis configured and accessible
- [ ] Fee structures configured in database
- [ ] Environment variables set
- [ ] Monitoring configured
- [ ] Alerts configured
- [ ] Load testing completed

### Deployment Steps

1. **Deploy to staging**
   ```bash
   # Build release
   cargo build --release --features database,cache
   
   # Deploy to staging
   ./deploy-staging.sh
   ```

2. **Run smoke tests**
   ```bash
   ./run-smoke-tests.sh staging
   ```

3. **Monitor metrics**
   - Cache hit rate
   - Response times
   - Error rates

4. **Deploy to production**
   ```bash
   ./deploy-production.sh
   ```

## Troubleshooting

### Rate not found
**Problem**: Service returns "Rate not found" error

**Solution**:
1. Check if FixedRateProvider is added to service
2. Verify currency pair is supported
3. Check database for stored rates

### Cache misses
**Problem**: High cache miss rate

**Solution**:
1. Verify Redis is running: `redis-cli ping`
2. Check cache configuration
3. Monitor cache TTL settings
4. Check Redis memory usage

### Incorrect fee calculations
**Problem**: Fees don't match expected values

**Solution**:
1. Verify fee structures in database
2. Check fee service integration
3. Review fee calculation logic
4. Test with known amounts

### Performance issues
**Problem**: Slow response times

**Solution**:
1. Check cache hit rate (should be > 95%)
2. Monitor database connection pool
3. Check Redis latency
4. Review query performance

## Best Practices

1. **Always validate input amounts** before calling the service
2. **Store the exchange rate** with each transaction for audit trail
3. **Monitor cache hit rate** to ensure caching is effective
4. **Set up alerts** for rate validation failures
5. **Test rounding behavior** at various amounts
6. **Document fee calculations** for transparency
7. **Use BigDecimal** for all monetary calculations

## Support

For issues or questions:
- Check [full documentation](EXCHANGE_RATE_SERVICE.md)
- Review [quick start guide](EXCHANGE_RATE_QUICK_START.md)
- See [usage examples](../examples/exchange_rate_service_example.rs)
- Report issues on GitHub

## Next Steps

1. Integrate with payment providers
2. Add rate history API endpoints
3. Implement rate alerts for users
4. Add multi-currency support
5. Integrate external rate APIs

---

**Integration Status**: Ready for production use ✅
