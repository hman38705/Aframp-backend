-- Enhanced fee structures for tiered, provider-specific fees
-- Supports Issue #44: Dynamic Fee Calculation Service

-- Rename fee_type to transaction_type before any references to transaction_type
ALTER TABLE fee_structures RENAME COLUMN fee_type TO transaction_type;

-- Add new columns to fee_structures for tiered and provider-specific fees
ALTER TABLE fee_structures 
ADD COLUMN IF NOT EXISTS payment_provider TEXT,
ADD COLUMN IF NOT EXISTS payment_method TEXT,
ADD COLUMN IF NOT EXISTS min_amount NUMERIC(36, 18),
ADD COLUMN IF NOT EXISTS max_amount NUMERIC(36, 18),
ADD COLUMN IF NOT EXISTS provider_fee_percent NUMERIC(10, 4),
ADD COLUMN IF NOT EXISTS provider_fee_flat NUMERIC(36, 18) DEFAULT 0,
ADD COLUMN IF NOT EXISTS provider_fee_cap NUMERIC(36, 18),
ADD COLUMN IF NOT EXISTS platform_fee_percent NUMERIC(10, 4),
ADD COLUMN IF NOT EXISTS platform_fee_flat NUMERIC(36, 18) DEFAULT 0;

-- Add check constraints
ALTER TABLE fee_structures 
ADD CONSTRAINT chk_fee_amount_range CHECK (min_amount IS NULL OR max_amount IS NULL OR min_amount <= max_amount),
ADD CONSTRAINT chk_provider_fee_percent CHECK (provider_fee_percent IS NULL OR (provider_fee_percent >= 0 AND provider_fee_percent <= 100)),
ADD CONSTRAINT chk_platform_fee_percent CHECK (platform_fee_percent IS NULL OR (platform_fee_percent >= 0 AND platform_fee_percent <= 100));

-- Create index for efficient tier lookup
CREATE INDEX IF NOT EXISTS idx_fee_structures_tier_lookup 
ON fee_structures(transaction_type, payment_provider, payment_method, is_active, min_amount, max_amount)
WHERE is_active = TRUE;

-- Update check constraint for transaction_type
ALTER TABLE fee_structures DROP CONSTRAINT IF EXISTS fee_structures_fee_type_check;
ALTER TABLE fee_structures 
ADD CONSTRAINT fee_structures_transaction_type_check 
CHECK (transaction_type IN ('onramp', 'offramp', 'bill_payment', 'exchange', 'transfer'));

-- Create audit log table for fee calculations
CREATE TABLE IF NOT EXISTS fee_calculation_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID,
    transaction_type TEXT NOT NULL,
    amount NUMERIC(36, 18) NOT NULL,
    currency TEXT NOT NULL,
    payment_provider TEXT,
    payment_method TEXT,
    fee_structure_id UUID REFERENCES fee_structures(id),
    provider_fee NUMERIC(36, 18) NOT NULL DEFAULT 0,
    platform_fee NUMERIC(36, 18) NOT NULL DEFAULT 0,
    stellar_fee_xlm NUMERIC(36, 18) NOT NULL DEFAULT 0,
    stellar_fee_ngn NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fees NUMERIC(36, 18) NOT NULL,
    net_amount NUMERIC(36, 18) NOT NULL,
    effective_rate NUMERIC(10, 4),
    calculation_metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_fee_calculation_logs_transaction 
ON fee_calculation_logs(transaction_id);

CREATE INDEX IF NOT EXISTS idx_fee_calculation_logs_created 
ON fee_calculation_logs(created_at DESC);

COMMENT ON TABLE fee_calculation_logs IS 'Audit log for all fee calculations';
COMMENT ON COLUMN fee_calculation_logs.effective_rate IS 'Total fee as percentage of amount';
