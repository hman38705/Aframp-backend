-- Seed initial fee structures for Aframp
-- Run this after migrations to populate default fee configurations

-- Clear existing fee structures (optional - comment out in production)
-- DELETE FROM fee_structures;

-- ============================================================================
-- ONRAMP FEES (Buy cNGN with NGN)
-- ============================================================================

-- Flutterwave Card Payments
-- Tier 1: Small amounts (₦1,000 - ₦50,000)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'flutterwave', 'card', 1000, 50000, 1.4, 100, 2000, 0.5, 0, true, 
 '{"description": "Flutterwave card - small amounts", "tier": 1}'::jsonb);

-- Tier 2: Medium amounts (₦50,001 - ₦500,000)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'flutterwave', 'card', 50001, 500000, 1.4, 0, 2000, 0.3, 0, true,
 '{"description": "Flutterwave card - medium amounts", "tier": 2}'::jsonb);

-- Tier 3: Large amounts (₦500,001+)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'flutterwave', 'card', 500001, NULL, 1.4, 0, 2000, 0.2, 0, true,
 '{"description": "Flutterwave card - large amounts", "tier": 3}'::jsonb);

-- Flutterwave Bank Transfer
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'flutterwave', 'bank_transfer', 1000, NULL, 0.8, 50, 5000, 0.3, 0, true,
 '{"description": "Flutterwave bank transfer", "min_fee": 50, "max_fee": 5000}'::jsonb);

-- Flutterwave USSD
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'flutterwave', 'ussd', 1000, NULL, 0, 50, NULL, 0.3, 0, true,
 '{"description": "Flutterwave USSD - flat fee"}'::jsonb);

-- Paystack Card Payments
-- Tier 1: Small amounts (₦1,000 - ₦50,000)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'paystack', 'card', 1000, 50000, 1.5, 0, 2000, 0.5, 0, true,
 '{"description": "Paystack card - small amounts", "tier": 1}'::jsonb);

-- Tier 2: Medium amounts (₦50,001 - ₦500,000)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'paystack', 'card', 50001, 500000, 1.5, 0, 2000, 0.3, 0, true,
 '{"description": "Paystack card - medium amounts", "tier": 2}'::jsonb);

-- Tier 3: Large amounts (₦500,001+)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'paystack', 'card', 500001, NULL, 1.5, 0, 2000, 0.2, 0, true,
 '{"description": "Paystack card - large amounts", "tier": 3}'::jsonb);

-- Paystack Bank Transfer
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'paystack', 'bank_transfer', 1000, NULL, 0, 50, NULL, 0.3, 0, true,
 '{"description": "Paystack bank transfer - flat fee"}'::jsonb);

-- ============================================================================
-- OFFRAMP FEES (Sell cNGN for NGN)
-- ============================================================================

-- Flutterwave Bank Transfer (Payout)
-- Tier 1: Small amounts
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('offramp', 'flutterwave', 'bank_transfer', 1000, 50000, 0.8, 50, 5000, 0.5, 0, true,
 '{"description": "Flutterwave payout - small amounts", "tier": 1}'::jsonb);

-- Tier 2: Medium amounts
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('offramp', 'flutterwave', 'bank_transfer', 50001, 500000, 0.8, 50, 5000, 0.3, 0, true,
 '{"description": "Flutterwave payout - medium amounts", "tier": 2}'::jsonb);

-- Tier 3: Large amounts
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('offramp', 'flutterwave', 'bank_transfer', 500001, NULL, 0.8, 50, 5000, 0.2, 0, true,
 '{"description": "Flutterwave payout - large amounts", "tier": 3}'::jsonb);

-- Paystack Bank Transfer (Payout)
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('offramp', 'paystack', 'bank_transfer', 1000, NULL, 0, 50, NULL, 0.5, 0, true,
 '{"description": "Paystack payout - flat fee"}'::jsonb);

-- ============================================================================
-- BILL PAYMENT FEES
-- ============================================================================

-- Generic bill payment fees
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('bill_payment', NULL, NULL, 100, NULL, 0.5, 50, 1000, 0.1, 0, true,
 '{"description": "Bill payment convenience fee"}'::jsonb);

-- ============================================================================
-- INTERNATIONAL CARD FEES (Higher rates)
-- ============================================================================

-- Flutterwave International Cards
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'flutterwave', 'international_card', 1000, NULL, 3.8, 0, NULL, 0.5, 0, true,
 '{"description": "Flutterwave international cards", "region": "international"}'::jsonb);

-- Paystack International Cards
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, min_amount, max_amount,
 provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, 
 platform_fee_flat, is_active, metadata)
VALUES 
('onramp', 'paystack', 'international_card', 1000, NULL, 3.9, 0, NULL, 0.5, 0, true,
 '{"description": "Paystack international cards", "region": "international"}'::jsonb);

-- Verify inserted records
SELECT 
    transaction_type,
    payment_provider,
    payment_method,
    min_amount,
    max_amount,
    provider_fee_percent,
    provider_fee_flat,
    platform_fee_percent,
    metadata->>'description' as description
FROM fee_structures
WHERE is_active = true
ORDER BY transaction_type, payment_provider, min_amount;
