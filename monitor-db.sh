#!/bin/bash
# Database Monitoring Script

DB_NAME="aframp"

echo "=== Database Status ==="
psql -U aframp_user -d $DB_NAME -c "
SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size,
    n_live_tup as rows
FROM pg_stat_user_tables
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
"

echo ""
echo "=== Active Connections ==="
psql -U aframp_user -d $DB_NAME -c "
SELECT count(*) as active_connections 
FROM pg_stat_activity 
WHERE state = 'active';
"

echo ""
echo "=== Slow Queries (>1s) ==="
psql -U aframp_user -d $DB_NAME -c "
SELECT 
    query,
    calls,
    total_time,
    mean_time,
    max_time
FROM pg_stat_statements
WHERE mean_time > 1000
ORDER BY mean_time DESC
LIMIT 10;
"

echo ""
echo "=== Cache Hit Ratio ==="
psql -U aframp_user -d $DB_NAME -c "
SELECT 
    sum(heap_blks_read) as heap_read,
    sum(heap_blks_hit) as heap_hit,
    sum(heap_blks_hit) / (sum(heap_blks_hit) + sum(heap_blks_read)) as ratio
FROM pg_statio_user_tables;
"
