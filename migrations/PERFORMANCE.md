# Database Performance Configuration

## Connection Pool Settings

Configure in your `.env` file:

```env
# Database connection pool settings (for optimal performance)
DATABASE_MAX_CONNECTIONS=20
DATABASE_MIN_CONNECTIONS=5
DATABASE_ACQUIRE_TIMEOUT=30
DATABASE_IDLE_TIMEOUT=600
DATABASE_MAX_LIFETIME=1800
```

### Recommended Pool Sizes by Deployment:

**Development:**
- MAX_CONNECTIONS: 10
- MIN_CONNECTIONS: 2

**Staging:**
- MAX_CONNECTIONS: 20
- MIN_CONNECTIONS: 5

**Production:**
- MAX_CONNECTIONS: 50-100 (based on load testing)
- MIN_CONNECTIONS: 10

### Connection Pool Formula:
```
connections = ((core_count × 2) + effective_spindle_count)
```

For most web applications:
- Start with 20 connections
- Monitor with `pg_stat_activity`
- Adjust based on actual usage

### Monitoring Connection Pool

```sql
-- Check current connections
SELECT count(*) FROM pg_stat_activity;

-- Check idle connections
SELECT count(*) FROM pg_stat_activity WHERE state = 'idle';

-- Check active queries
SELECT count(*) FROM pg_stat_activity WHERE state = 'active';
```

## Query Performance Baselines

Run these queries after migration to establish baselines:

```sql
-- Wallet balance lookup (Target: < 10ms)
EXPLAIN ANALYZE 
SELECT balance FROM wallets 
WHERE wallet_address = 'GXXXXXX' AND chain = 'stellar';

-- Transaction status check (Target: < 5ms)
EXPLAIN ANALYZE 
SELECT status FROM transactions 
WHERE wallet_address = 'GXXXXXX' AND status = 'completed';

-- Recent transactions (Target: < 20ms)
EXPLAIN ANALYZE 
SELECT * FROM transactions 
WHERE wallet_address = 'GXXXXXX' 
ORDER BY created_at DESC 
LIMIT 50;

-- Exchange rate lookup (Target: < 5ms)
EXPLAIN ANALYZE 
SELECT rate FROM exchange_rates 
WHERE from_currency = 'NGN' AND to_currency = 'AFRI'
ORDER BY valid_until DESC 
LIMIT 1;

-- User summary (Target: < 5ms)
EXPLAIN ANALYZE 
SELECT * FROM user_transaction_summary 
WHERE user_id = 'uuid';
```

## Index Maintenance

### Monitor Index Usage
```sql
-- Find unused indexes
SELECT 
    schemaname, tablename, indexname, idx_scan
FROM pg_stat_user_indexes 
WHERE idx_scan = 0 
AND schemaname = 'public'
ORDER BY pg_relation_size(indexrelid) DESC;

-- Index size and usage
SELECT 
    tablename,
    indexname,
    idx_scan as index_scans,
    pg_size_pretty(pg_relation_size(indexrelid)) as index_size
FROM pg_stat_user_indexes
WHERE schemaname = 'public'
ORDER BY pg_relation_size(indexrelid) DESC;
```

### Refresh Materialized Views

Set up cron jobs or scheduled tasks:

```bash
# Every 5 minutes - user transaction summary
*/5 * * * * psql $DATABASE_URL -c "REFRESH MATERIALIZED VIEW CONCURRENTLY user_transaction_summary;"

# Once per day - daily volume
0 1 * * * psql $DATABASE_URL -c "REFRESH MATERIALIZED VIEW CONCURRENTLY daily_transaction_volume;"
```

## Partition Management

### Create New Monthly Partition

```sql
-- Run at the beginning of each month
CREATE TABLE transactions_2026_04 PARTITION OF transactions_partitioned
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
```

### Archive Old Partitions

```sql
-- Detach old partition (older than 1 year)
ALTER TABLE transactions_partitioned 
DETACH PARTITION transactions_2025_01;

-- Move to archive schema
CREATE SCHEMA IF NOT EXISTS archive;
ALTER TABLE transactions_2025_01 SET SCHEMA archive;
```

## Performance Tuning

### PostgreSQL Configuration

Add to `postgresql.conf`:

```conf
# Memory
shared_buffers = 256MB
effective_cache_size = 1GB
work_mem = 16MB

# Connections
max_connections = 100

# Query Planning
random_page_cost = 1.1  # For SSD
effective_io_concurrency = 200

# Logging
log_min_duration_statement = 1000  # Log queries > 1s
log_line_prefix = '%t [%p]: '
log_statement = 'mod'  # Log DDL
```

### Enable Query Statistics

```sql
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- View slowest queries
SELECT 
    calls,
    mean_exec_time,
    query
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;
```
