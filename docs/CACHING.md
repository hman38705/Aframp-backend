# Redis Caching Layer Documentation

## Overview

Aframp implements a robust Redis-based caching layer to dramatically improve performance, reduce database load, and ensure reliable scaling. The caching system provides sub-millisecond response times for frequently accessed data while maintaining full system functionality even when Redis is unavailable.

## Architecture

### Core Components

1. **Cache Module** (`src/cache/`)
   - `mod.rs`: Connection pool management and configuration
   - `cache.rs`: Generic cache trait and Redis implementation
   - `keys.rs`: Type-safe cache key builders
   - `error.rs`: Cache-specific error handling

2. **Repository Integration**
   - Exchange rates cached for 1.5 minutes
   - Wallet balances cached for 45 seconds
   - Trustline existence cached for 1 hour
   - Automatic cache invalidation on data updates

### Cache Strategy

- **Cache-First**: Check cache before database queries
- **Write-Through**: Update database first, then invalidate cache
- **Graceful Degradation**: Full functionality without Redis
- **Type Safety**: Compile-time key validation

## Configuration

### Environment Variables

```bash
# Redis Connection
REDIS_URL=redis://127.0.0.1:6379
REDIS_MAX_CONNECTIONS=20
REDIS_MIN_IDLE=5
REDIS_CONNECTION_TIMEOUT=5
REDIS_MAX_LIFETIME=300
REDIS_IDLE_TIMEOUT=60
REDIS_HEALTH_CHECK_INTERVAL=30
```

### Cache TTL Values

| Data Type | TTL | Purpose |
|-----------|-----|---------|
| Exchange Rates | 90 seconds | Frequently updated, shared across users |
| Wallet Balances | 45 seconds | Real-time sensitive, invalidated on transactions |
| Trustlines | 1 hour | Rarely change after creation |
| Fee Structures | 1 hour | Static for extended periods |
| User Sessions | 30 minutes | Authentication state |
| Transaction Status | 5 minutes | Temporary status tracking |
| Recent Transactions | 10 minutes | User-specific history |
| JWT Validation | 15 minutes | Token verification |
| Rate Limiting | 1 minute | Security counters |
| Bill Providers | 30 minutes | Provider configurations |

## Key Naming Convention

All cache keys follow the pattern: `v1:{namespace}:{identifier}`

### Examples

```
v1:wallet:balance:GA123456789
v1:rate:CNGN:USD
v1:wallet:trustline:GA123456789
v1:rate:convert:100.50:CNGN:USD
v1:auth:session:session_123
v1:auth:rate_limit:user_123:login
v1:bill:provider:provider_456
```

## Usage Examples

### Basic Cache Operations

```rust
use aframp_backend::cache::{Cache, CacheConfig, RedisCache};

// Initialize cache
let config = CacheConfig::default();
let pool = init_cache_pool(config).await?;
let cache = RedisCache::new(pool);

// Store and retrieve data
cache.set("my:key", &"my_value".to_string(), Some(Duration::from_secs(300))).await?;
let value: Option<String> = cache.get("my:key").await?;
```

### Repository with Caching

```rust
// Create repository with caching enabled
let db_pool = init_db_pool().await?;
let cache_pool = init_cache_pool(config).await?;
let cache = RedisCache::new(cache_pool);

let repo = ExchangeRateRepository::with_cache(db_pool, cache);

// Automatic caching on operations
let rate = repo.get_current_rate("CNGN", "USD").await?; // Checks cache first
repo.upsert_rate("CNGN", "USD", "0.85", None).await?; // Invalidates cache
```

### Type-Safe Keys

```rust
use aframp_backend::cache::keys::*;

// Type-safe key generation
let balance_key = wallet::BalanceKey::new("GA123456789");
let rate_key = exchange_rate::CurrencyPairKey::cngn_rate("USD");

// Keys implement Display for string conversion
println!("{}", balance_key); // "v1:wallet:balance:GA123456789"
```

## Performance Expectations

### Cache Hit Rates (Target)

- **Wallet Balances**: ≥ 70% hit rate
- **Exchange Rates**: ≥ 90% hit rate
- **Trustlines**: ≥ 95% hit rate

### Response Times

- **Cache Hit**: < 2ms
- **Cache Miss**: 10-50ms (database dependent)
- **Cache Write**: < 5ms

### Load Reduction

- **Database Queries**: 60-80% reduction
- **Stellar API Calls**: 70-90% reduction
- **Overall Response Time**: 5-10x improvement

## Cache Invalidation Strategies

### Automatic Invalidation

1. **Transaction Completion**
   - Sender and receiver wallet balances invalidated
   - Trustline balances updated

2. **Exchange Rate Updates**
   - All currency pair caches invalidated
   - Conversion calculations cleared

3. **Trustline Creation**
   - Positive existence result cached immediately

4. **Balance Updates**
   - Specific wallet balance cache invalidated

### Manual Invalidation

```rust
// Pattern-based invalidation
cache.delete_pattern("v1:wallet:balance:*").await?;

// Specific key invalidation
cache.delete(&balance_key.to_string()).await?;
```

## Monitoring and Metrics

### Health Checks

```rust
// Cache health check
let is_healthy = health_check(&cache_pool).await.is_ok();

// Pool statistics
let stats = get_cache_stats(&cache_pool);
println!("Connections: {}, Idle: {}", stats.connections, stats.idle_connections);
```

### Key Metrics to Monitor

1. **Hit/Miss Ratios** by data type
2. **Cache Memory Usage**
3. **Connection Pool Utilization**
4. **Cache Operation Latencies**
5. **Invalidation Frequency**

## Failure Scenarios

### Redis Unavailable

- **Behavior**: Graceful degradation to database-only operation
- **Impact**: Slower responses, increased database load
- **Recovery**: Automatic reconnection when Redis recovers

### Cache Inconsistency

- **Prevention**: Write-through strategy ensures data consistency
- **Detection**: Monitor for cache/database mismatches
- **Recovery**: Cache invalidation on detected inconsistencies

### Memory Pressure

- **TTL Expiration**: Automatic cleanup of stale data
- **LRU Eviction**: Redis handles memory pressure automatically
- **Monitoring**: Track memory usage and key counts

## Testing

### Unit Tests

```bash
# Run cache unit tests
cargo test --features cache cache::
```

### Integration Tests

```bash
# Run with Redis and database
REDIS_URL=redis://localhost:6379 DATABASE_URL=postgres://... cargo test --features cache --test cache_integration_test
```

### Load Testing

```bash
# Simulate high concurrency
cargo test --features cache -- --nocapture load_test
```

## Best Practices

### Cache Key Design

1. **Versioning**: Include `v1:` prefix for future migrations
2. **Hierarchy**: Use colons for logical separation
3. **Consistency**: Follow established naming patterns
4. **Uniqueness**: Ensure no key collisions

### Error Handling

1. **Graceful Degradation**: Never fail operations due to cache issues
2. **Logging**: Log cache misses and failures for monitoring
3. **Timeouts**: Configure appropriate timeouts for cache operations
4. **Circuit Breakers**: Implement if cache becomes consistently unavailable

### Performance Optimization

1. **Batch Operations**: Use `set_multiple` and `get_multiple` for bulk operations
2. **TTL Strategy**: Choose appropriate TTL values based on data volatility
3. **Key Compression**: Consider shorter key names for high-frequency operations
4. **Connection Pooling**: Maintain optimal pool sizes

## Troubleshooting

### Common Issues

1. **Connection Failures**
   - Check Redis server status
   - Verify connection string
   - Review firewall settings

2. **Memory Issues**
   - Monitor Redis memory usage
   - Adjust TTL values
   - Implement key eviction policies

3. **Performance Degradation**
   - Check cache hit rates
   - Monitor connection pool utilization
   - Profile cache operation latencies

### Debug Commands

```bash
# Redis CLI debugging
redis-cli
> KEYS "v1:*"          # List all cache keys
> TTL "v1:wallet:balance:GA123"  # Check key TTL
> INFO memory          # Memory usage statistics
> INFO stats           # Hit/miss statistics
```

## Future Enhancements

1. **Cache Warming**: Pre-populate frequently accessed data on startup
2. **Cache Analytics**: Detailed metrics and performance dashboards
3. **Distributed Caching**: Redis Cluster support for horizontal scaling
4. **Cache Compression**: Reduce memory usage for large values
5. **Smart TTL**: Dynamic TTL adjustment based on access patterns