-- Migration: Database Indexes, Constraints, and Performance Optimizations
-- Purpose: Add comprehensive indexes and constraints to ensure efficient query performance
--          and data integrity as transaction volume scales
-- Related: Issue #4

-- ============================================================================
-- UP MIGRATION
-- ============================================================================

-- ============================================================================
-- TRANSACTION INDEXES
-- ============================================================================

-- Composite index for common wallet transaction queries (status filtering)
CREATE INDEX IF NOT EXISTS idx_transactions_wallet_status 
ON transactions(wallet_address, status);

-- Index for time-based queries (recent transactions)
CREATE INDEX IF NOT EXISTS idx_transactions_created_at 
ON transactions(created_at DESC);

-- Partial index for payment reference lookups (only when reference exists)
CREATE INDEX IF NOT EXISTS idx_transactions_payment_ref 
ON transactions(payment_reference) 
WHERE payment_reference IS NOT NULL;

-- Index for transaction type filtering
CREATE INDEX IF NOT EXISTS idx_transactions_type_status 
ON transactions(transaction_type, status);

-- ============================================================================
-- WALLET INDEXES
-- ============================================================================

-- Composite index for wallet lookups by address and chain
CREATE INDEX IF NOT EXISTS idx_wallets_address_chain 
ON wallets(wallet_address, chain);

-- Index for user wallet queries
CREATE INDEX IF NOT EXISTS idx_wallets_user_id 
ON wallets(user_id);

-- ============================================================================
-- WEBHOOK INDEXES
-- ============================================================================

-- Partial index for processing pending webhooks efficiently
CREATE INDEX IF NOT EXISTS idx_webhooks_unprocessed 
ON webhook_events(created_at) 
WHERE status = 'pending';

-- Index for webhook event lookups by transaction
CREATE INDEX IF NOT EXISTS idx_webhooks_transaction 
ON webhook_events(transaction_id);

-- GIN index for JSONB payload searches
CREATE INDEX IF NOT EXISTS idx_webhooks_payload_gin 
ON webhook_events USING GIN (payload);

-- ============================================================================
-- AFRI OPERATIONS INDEXES
-- ============================================================================

-- Composite index for trustline queries by wallet and status
CREATE INDEX IF NOT EXISTS idx_trustlines_wallet 
ON trustline_operations(wallet_address, status);

-- Index for recent trustline operations
CREATE INDEX IF NOT EXISTS idx_trustlines_created_at 
ON trustline_operations(created_at DESC);

-- ============================================================================
-- EXCHANGE RATE INDEXES
-- ============================================================================

-- Index for quick currency pair lookups
CREATE INDEX IF NOT EXISTS idx_exchange_rates_currency_pair 
ON exchange_rates(from_currency, to_currency);

-- Index for getting latest rates
CREATE INDEX IF NOT EXISTS idx_exchange_rates_valid_until 
ON exchange_rates(valid_until DESC);

-- ============================================================================
-- USER INDEXES
-- ============================================================================

-- Index for email lookups (login)
CREATE INDEX IF NOT EXISTS idx_users_email 
ON users(email);

-- Index for KYC status filtering
CREATE INDEX IF NOT EXISTS idx_users_kyc_status 
ON users(kyc_status);

-- ============================================================================
-- CONSTRAINTS
-- ============================================================================

-- Transaction amount constraints
ALTER TABLE transactions 
ADD CONSTRAINT chk_transactions_amount_positive 
CHECK (amount >= 0);

ALTER TABLE transactions 
ADD CONSTRAINT chk_transactions_fee_positive 
CHECK (fee >= 0);

-- Transaction status enum constraint
ALTER TABLE transactions 
ADD CONSTRAINT chk_transactions_status 
CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'cancelled'));

-- Transaction type enum constraint
ALTER TABLE transactions 
ADD CONSTRAINT chk_transactions_type 
CHECK (transaction_type IN ('deposit', 'withdrawal', 'transfer', 'swap'));

-- Trustline operation status constraint
ALTER TABLE trustline_operations 
ADD CONSTRAINT chk_trustline_status 
CHECK (status IN ('pending', 'completed', 'failed'));

-- Exchange rate constraints
ALTER TABLE exchange_rates 
ADD CONSTRAINT chk_exchange_rate_positive 
CHECK (rate > 0);

-- User balance constraint
ALTER TABLE wallets 
ADD CONSTRAINT chk_wallets_balance_non_negative 
CHECK (balance >= 0);

-- Webhook event status constraint
ALTER TABLE webhook_events 
ADD CONSTRAINT chk_webhook_status 
CHECK (status IN ('pending', 'processing', 'completed', 'failed'));

-- KYC status constraint
ALTER TABLE users 
ADD CONSTRAINT chk_users_kyc_status 
CHECK (kyc_status IN ('pending', 'submitted', 'approved', 'rejected'));

-- Wallet address format constraints
-- Stellar addresses: 56 characters starting with 'G' (public key)
ALTER TABLE wallets
ADD CONSTRAINT chk_wallets_address_format
CHECK (
    LENGTH(wallet_address) = 56 AND
    wallet_address ~ '^G[A-Z2-7]{55}$'
);

-- Transaction wallet address validation
ALTER TABLE transactions
ADD CONSTRAINT chk_transactions_wallet_format
CHECK (
    LENGTH(wallet_address) = 56 AND
    wallet_address ~ '^G[A-Z2-7]{55}$'
);

-- Trustline wallet address validation
ALTER TABLE trustline_operations
ADD CONSTRAINT chk_trustline_wallet_format
CHECK (
    LENGTH(wallet_address) = 56 AND
    wallet_address ~ '^G[A-Z2-7]{55}$'
);

-- ============================================================================
-- TRIGGERS FOR UPDATED_AT TIMESTAMPS
-- ============================================================================

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply trigger to transactions table
CREATE TRIGGER update_transactions_updated_at 
BEFORE UPDATE ON transactions
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Apply trigger to wallets table
CREATE TRIGGER update_wallets_updated_at 
BEFORE UPDATE ON wallets
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Apply trigger to users table
CREATE TRIGGER update_users_updated_at 
BEFORE UPDATE ON users
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Apply trigger to trustline_operations table
CREATE TRIGGER update_trustline_operations_updated_at 
BEFORE UPDATE ON trustline_operations
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Apply trigger to webhook_events table
CREATE TRIGGER update_webhook_events_updated_at 
BEFORE UPDATE ON webhook_events
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- TABLE PARTITIONING
-- ============================================================================

-- Partition transactions table by month for better performance with large datasets
-- This improves query performance for time-based queries and enables easier archival

CREATE TABLE IF NOT EXISTS transactions_partitioned (
    LIKE transactions INCLUDING ALL
) PARTITION BY RANGE (created_at);

-- Create partitions for current and upcoming months
-- In production, automate partition creation with a scheduled job

CREATE TABLE IF NOT EXISTS transactions_2026_01 PARTITION OF transactions_partitioned
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');

CREATE TABLE IF NOT EXISTS transactions_2026_02 PARTITION OF transactions_partitioned
    FOR VALUES FROM ('2026-02-01') TO ('2026-03-01');

CREATE TABLE IF NOT EXISTS transactions_2026_03 PARTITION OF transactions_partitioned
    FOR VALUES FROM ('2026-03-01') TO ('2026-04-01');

-- Create default partition for data outside defined ranges
CREATE TABLE IF NOT EXISTS transactions_default PARTITION OF transactions_partitioned DEFAULT;

-- Note: To migrate existing data, use:
-- INSERT INTO transactions_partitioned SELECT * FROM transactions;
-- Then rename tables after verification

-- ============================================================================
-- MATERIALIZED VIEWS
-- ============================================================================

-- Materialized view for user transaction summaries
-- Provides fast access to aggregate transaction data per user
-- Refresh periodically (e.g., every 5 minutes via cron job)

CREATE MATERIALIZED VIEW IF NOT EXISTS user_transaction_summary AS
SELECT 
    w.user_id,
    w.wallet_address,
    COUNT(t.id) as total_transactions,
    COUNT(t.id) FILTER (WHERE t.status = 'completed') as completed_transactions,
    COUNT(t.id) FILTER (WHERE t.status = 'pending') as pending_transactions,
    COUNT(t.id) FILTER (WHERE t.status = 'failed') as failed_transactions,
    SUM(t.amount) FILTER (WHERE t.status = 'completed' AND t.transaction_type = 'deposit') as total_deposited,
    SUM(t.amount) FILTER (WHERE t.status = 'completed' AND t.transaction_type = 'withdrawal') as total_withdrawn,
    MAX(t.created_at) as last_transaction_at,
    w.balance as current_balance
FROM wallets w
LEFT JOIN transactions t ON t.wallet_address = w.wallet_address
GROUP BY w.user_id, w.wallet_address, w.balance;

-- Index on materialized view for fast user lookups
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_tx_summary_user_id 
ON user_transaction_summary(user_id, wallet_address);

-- Index for sorting by last transaction
CREATE INDEX IF NOT EXISTS idx_user_tx_summary_last_tx 
ON user_transaction_summary(last_transaction_at DESC);

-- Materialized view for daily transaction volume
CREATE MATERIALIZED VIEW IF NOT EXISTS daily_transaction_volume AS
SELECT 
    DATE(created_at) as transaction_date,
    transaction_type,
    status,
    COUNT(*) as transaction_count,
    SUM(amount) as total_volume,
    AVG(amount) as avg_transaction_size
FROM transactions
WHERE created_at >= CURRENT_DATE - INTERVAL '90 days'
GROUP BY DATE(created_at), transaction_type, status
ORDER BY transaction_date DESC;

-- Index for quick date lookups
CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_volume_date 
ON daily_transaction_volume(transaction_date DESC, transaction_type, status);

-- ============================================================================
-- PERFORMANCE NOTES
-- ============================================================================

-- Expected Performance Targets:
-- - Wallet balance lookup: < 10ms (idx_wallets_address_chain)
-- - Transaction status check: < 5ms (idx_transactions_wallet_status)
-- - Recent transactions query (last 50): < 20ms (idx_transactions_created_at)
-- - Exchange rate lookup: < 5ms (idx_exchange_rates_currency_pair)
-- - User summary lookup: < 5ms (user_transaction_summary materialized view)

-- Materialized View Refresh Strategy:
-- Refresh user_transaction_summary every 5 minutes:
-- REFRESH MATERIALIZED VIEW CONCURRENTLY user_transaction_summary;
-- 
-- Refresh daily_transaction_volume once per day:
-- REFRESH MATERIALIZED VIEW CONCURRENTLY daily_transaction_volume;

-- Index Usage Monitoring:
-- Use pg_stat_user_indexes to monitor index usage and identify unused indexes
-- Query: SELECT * FROM pg_stat_user_indexes WHERE schemaname = 'public';

-- Query Performance Monitoring:
-- Enable pg_stat_statements extension for query performance tracking
-- CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- Partition Management:
-- Automate monthly partition creation with a scheduled job
-- Archive old partitions by detaching and moving to archive schema:
-- ALTER TABLE transactions_partitioned DETACH PARTITION transactions_2025_01;

-- Example EXPLAIN ANALYZE results for critical queries:
-- 
-- Query: Get user's recent transactions
-- EXPLAIN ANALYZE SELECT * FROM transactions 
-- WHERE wallet_address = 'GXXX...' AND status = 'completed' 
-- ORDER BY created_at DESC LIMIT 50;
-- Expected: Index Scan using idx_transactions_wallet_status (cost=0.42..123.45)
--
-- Query: Get wallet balance
-- EXPLAIN ANALYZE SELECT balance FROM wallets 
-- WHERE wallet_address = 'GXXX...' AND chain = 'stellar';
-- Expected: Index Scan using idx_wallets_address_chain (cost=0.28..8.30)

-- ============================================================================
-- DOWN MIGRATION
-- ============================================================================

-- Drop materialized views
DROP MATERIALIZED VIEW IF EXISTS daily_transaction_volume;
DROP MATERIALIZED VIEW IF EXISTS user_transaction_summary;

-- Drop partitioned tables
DROP TABLE IF EXISTS transactions_default;
DROP TABLE IF EXISTS transactions_2026_03;
DROP TABLE IF EXISTS transactions_2026_02;
DROP TABLE IF EXISTS transactions_2026_01;
DROP TABLE IF EXISTS transactions_partitioned;

-- Drop triggers
DROP TRIGGER IF EXISTS update_webhook_events_updated_at ON webhook_events;
DROP TRIGGER IF EXISTS update_trustline_operations_updated_at ON trustline_operations;
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
DROP TRIGGER IF EXISTS update_wallets_updated_at ON wallets;
DROP TRIGGER IF EXISTS update_transactions_updated_at ON transactions;

-- Drop trigger function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop constraints
ALTER TABLE trustline_operations DROP CONSTRAINT IF EXISTS chk_trustline_wallet_format;
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS chk_transactions_wallet_format;
ALTER TABLE wallets DROP CONSTRAINT IF EXISTS chk_wallets_address_format;
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_kyc_status;
ALTER TABLE webhook_events DROP CONSTRAINT IF EXISTS chk_webhook_status;
ALTER TABLE wallets DROP CONSTRAINT IF EXISTS chk_wallets_balance_non_negative;
ALTER TABLE exchange_rates DROP CONSTRAINT IF EXISTS chk_exchange_rate_positive;
ALTER TABLE trustline_operations DROP CONSTRAINT IF EXISTS chk_trustline_status;
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS chk_transactions_type;
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS chk_transactions_status;
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS chk_transactions_fee_positive;
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS chk_transactions_amount_positive;

-- Drop indexes
DROP INDEX IF EXISTS idx_daily_volume_date;
DROP INDEX IF EXISTS idx_user_tx_summary_last_tx;
DROP INDEX IF EXISTS idx_user_tx_summary_user_id;
DROP INDEX IF EXISTS idx_users_kyc_status;
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_exchange_rates_valid_until;
DROP INDEX IF EXISTS idx_exchange_rates_currency_pair;
DROP INDEX IF EXISTS idx_trustlines_created_at;
DROP INDEX IF EXISTS idx_trustlines_wallet;
DROP INDEX IF EXISTS idx_webhooks_payload_gin;
DROP INDEX IF EXISTS idx_webhooks_transaction;
DROP INDEX IF EXISTS idx_webhooks_unprocessed;
DROP INDEX IF EXISTS idx_wallets_user_id;
DROP INDEX IF EXISTS idx_wallets_address_chain;
DROP INDEX IF EXISTS idx_transactions_type_status;
DROP INDEX IF EXISTS idx_transactions_payment_ref;
DROP INDEX IF EXISTS idx_transactions_created_at;
DROP INDEX IF EXISTS idx_transactions_wallet_status;
