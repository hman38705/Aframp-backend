# Rates API Integration Guide

## Quick Start

This guide shows how to integrate the Rates API endpoint into your Axum application.

## Step 1: Add to Router

In your `main.rs` or application setup:

```rust
use aframp_backend::api::rates::{get_rates, options_rates, RatesState};
use aframp_backend::cache::cache::RedisCache;
use aframp_backend::services::exchange_rate::ExchangeRateService;
use axum::{routing::{get, options}, Router};
use std::sync::Arc;

// ... your existing setup ...

// Create rates state
let rates_state = RatesState {
    exchange_rate_service: Arc::clone(&exchange_rate_service),
    cache: Some(Arc::clone(&redis_cache)),
};

// Add to router
let app = Router::new()
    // ... your existing routes ...
    .route("/api/rates", get(get_rates).options(options_rates))
    .with_state(rates_state);
```

## Step 2: Configure Dependencies

Ensure you have the required services initialized:

```rust
// Database connection
let db_pool = create_pool(&database_url).await?;

// Redis cache (optional but recommended)
let cache_config = CacheConfig {
    url: redis_url,
    max_connections: 10,
    min_idle: 2,
    connection_timeout_seconds: 5,
    max_lifetime_seconds: Some(300),
};
let cache_pool = init_cache_pool(cache_config).await?;
let redis_cache = Arc::new(RedisCache::new(cache_pool));

// Exchange rate service
let repository = ExchangeRateRepository::new(db_pool.clone());
let config = ExchangeRateServiceConfig::default();
let exchange_rate_service = ExchangeRateService::new(repository, config)
    .with_cache((*redis_cache).clone());
let exchange_rate_service = Arc::new(exchange_rate_service);
```

## Step 3: Environment Variables

Add to your `.env` file:

```bash
# Database
DATABASE_URL=postgresql://user:password@localhost/aframp

# Redis (optional but recommended for caching)
REDIS_URL=redis://localhost:6379

# Server
HOST=0.0.0.0
PORT=3000
```

## Step 4: Test the Endpoint

Start your server and test:

```bash
# Single pair
curl "http://localhost:3000/api/rates?from=NGN&to=cNGN"

# All pairs
curl "http://localhost:3000/api/rates"
```

## Complete Example

Here's a complete minimal example:

```rust
use aframp_backend::api::rates::{get_rates, options_rates, RatesState};
use aframp_backend::cache::cache::RedisCache;
use aframp_backend::cache::{init_cache_pool, CacheConfig};
use aframp_backend::database::connection::create_pool;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use aframp_backend::services::exchange_rate::{
    ExchangeRateService, ExchangeRateServiceConfig
};
use axum::{routing::{get, options}, Router};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment
    dotenv::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Database
    let database_url = std::env::var("DATABASE_URL")?;
    let db_pool = create_pool(&database_url).await?;

    // Cache (optional)
    let cache = if let Ok(redis_url) = std::env::var("REDIS_URL") {
        let config = CacheConfig {
            url: redis_url,
            max_connections: 10,
            min_idle: 2,
            connection_timeout_seconds: 5,
            max_lifetime_seconds: Some(300),
        };
        let pool = init_cache_pool(config).await?;
        Some(Arc::new(RedisCache::new(pool)))
    } else {
        None
    };

    // Exchange rate service
    let repository = ExchangeRateRepository::new(db_pool.clone());
    let config = ExchangeRateServiceConfig::default();
    let mut service = ExchangeRateService::new(repository, config);
    
    if let Some(ref c) = cache {
        service = service.with_cache((**c).clone());
    }
    
    let exchange_rate_service = Arc::new(service);

    // Rates state
    let rates_state = RatesState {
        exchange_rate_service,
        cache,
    };

    // Router
    let app = Router::new()
        .route("/api/rates", get(get_rates).options(options_rates))
        .with_state(rates_state);

    // Server
    let addr = "0.0.0.0:3000";
    println!("Server listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

## Without Redis Cache

If you don't have Redis available, you can run without caching:

```rust
let rates_state = RatesState {
    exchange_rate_service: Arc::clone(&exchange_rate_service),
    cache: None, // No caching
};
```

Note: Without caching, response times will be slower (50-100ms vs < 5ms).

## Testing

### Unit Tests

```bash
cargo test --lib api::rates
```

### Integration Tests

```bash
cargo test --test api_rates_test
```

### Manual Testing

```bash
# Start server
cargo run --example rates_api_demo

# In another terminal
curl "http://localhost:3000/api/rates?from=NGN&to=cNGN"
```

## Performance Tuning

### Cache Configuration

Adjust cache TTL based on your needs:

```rust
// In src/api/rates.rs, modify the TTL:
let ttl = Duration::from_secs(30); // Default: 30 seconds
```

### Connection Pools

Tune pool sizes for your load:

```rust
// Redis pool
let cache_config = CacheConfig {
    max_connections: 20,  // Increase for high traffic
    min_idle: 5,
    // ...
};

// Database pool
let db_pool = PgPoolOptions::new()
    .max_connections(20)
    .connect(&database_url)
    .await?;
```

## Monitoring

Add metrics collection:

```rust
use metrics::{counter, histogram};

// In get_rates handler
counter!("api.rates.requests").increment(1);
let start = std::time::Instant::now();

// ... handle request ...

histogram!("api.rates.duration").record(start.elapsed().as_secs_f64());
```

## Troubleshooting

### "Rate not found" errors

Ensure rates are seeded in the database:

```sql
INSERT INTO exchange_rates (from_currency, to_currency, rate, source)
VALUES 
  ('NGN', 'cNGN', '1.0', 'fixed_peg'),
  ('cNGN', 'NGN', '1.0', 'fixed_peg');
```

### Redis connection errors

Check Redis is running:

```bash
redis-cli ping
# Should return: PONG
```

### Slow responses

1. Enable Redis caching
2. Check database query performance
3. Monitor network latency
4. Review connection pool settings

## Next Steps

- [Read the full API documentation](./RATES_API.md)
- [Learn about caching strategy](./CACHING.md)
- [Explore exchange rate service](./EXCHANGE_RATE_SERVICE.md)
- [Set up monitoring and alerts](./MONITORING.md)
