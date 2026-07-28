-- Fix: consumer-id based transaction history queries were doing full table
-- scans on transaction tables added after 20260124000000_indexes_and_constraints.sql
-- because those newer tables never got a consumer-id index of their own.

-- batches already has a single-column index on initiated_by (idx_batches_initiated)
-- but consumer-scoped history lookups sort by created_at, so that index alone
-- still forces a sort/filter step. Add the composite index used by cursor
-- pagination, matching the pattern used for the transactions table.
CREATE INDEX IF NOT EXISTS idx_batches_initiated_by_created
    ON batches (initiated_by, created_at DESC);
