-- Migration: Update Travel Rule threshold to ₦1,500,000 / $1,000 (Issue: Travel Rule enforcement)
-- Raises the previously-seeded ₦500,000 threshold and adds an explicit onramp
-- threshold row (onramp checks are keyed on fiat currency "NGN", not "cNGN").

UPDATE travel_rule_thresholds
SET threshold_amount = 1500000, updated_at = NOW()
WHERE currency = 'cNGN'
  AND transaction_type IN ('cngn_transfer', 'offramp')
  AND jurisdiction = 'NG'
  AND threshold_amount = 500000;

INSERT INTO travel_rule_thresholds (currency, transaction_type, jurisdiction, threshold_amount)
VALUES
    ('NGN', 'onramp', 'NG', 1500000)
ON CONFLICT (currency, transaction_type, jurisdiction) DO UPDATE
    SET threshold_amount = EXCLUDED.threshold_amount, updated_at = NOW();

UPDATE travel_rule_unhosted_wallet_policy
SET threshold_amount = 1500000, updated_at = NOW()
WHERE threshold_currency = 'cNGN' AND threshold_amount = 500000;
