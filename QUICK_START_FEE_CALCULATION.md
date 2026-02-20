# Dynamic Fee Calculation Service - Quick Start Guide

## ✅ Implementation Complete

The dynamic fee calculation service has been successfully implemented for Issue #44.

## 🚀 Quick Setup

### 1. Run Migration and Seed Data

```bash
./setup-fee-structures.sh
```

This will:
- Run the enhanced fee structures migration
- Seed initial fee configurations
- Verify the setup

### 2. Verify Installation

```bash
# Check fee structures count
psql $DATABASE_URL -c "SELECT COUNT(*) FROM fee_structures WHERE is_active = true;"

# View all fee structures
psql $DATABASE_URL -c "SELECT transaction_type, payment_provider, payment_method, min_amount, max_amount, provider_fee_percent, platform_fee_percent FROM fee_structures WHERE is_active = true ORDER BY transaction_type, min_amount;"
```

## 📖 Usage Examples

### Basic Fee Calculation

```rust
use aframp_backend::services::fee_calculation::FeeCalculationService;
use sqlx::types::BigDecimal;
use std::str::FromStr;

// Create service
let service = FeeCalculationService::new(pool);

// Calculate fees
let breakdown = service.calculate_fees(
    "onramp",                                    // transaction type
    BigDecimal::from_str("100000").unwrap(),     // amount
    Some("flutterwave"),                         // provider
    Some("card"),                                // payment method
).await?;

// Access breakdown
println!("Total fees: ₦{}", breakdown.total);
println!("Net amount: ₦{}", breakdown.net_amount);
println!("Effective rate: {}%", breakdown.effective_rate);
```

### Fee Estimation (Without Provider)

```rust
let (min_fee, max_fee) = service.estimate_fees(
    "onramp",
    BigDecimal::from_str("100000").unwrap(),
).await?;

println!("Fee range: ₦{} - ₦{}", min_fee, max_fee);
```

### Fee Breakdown Structure

```json
{
  "amount": "100000",
  "currency": "NGN",
  "provider": {
    "name": "flutterwave",
    "method": "card",
    "percent": "1.4",
    "flat": "0",
    "cap": "2000",
    "calculated": "1400"
  },
  "platform": {
    "percent": "0.3",
    "calculated": "300"
  },
  "stellar": {
    "xlm": "0.00001",
    "ngn": "0",
    "absorbed": true
  },
  "total": "1700",
  "net_amount": "98300",
  "effective_rate": "1.7"
}
```

## 📊 Fee Tiers Overview

### Onramp (Buy cNGN)

| Tier | Amount Range | Provider Fee | Platform Fee | Effective Rate |
|------|-------------|--------------|--------------|----------------|
| 1 | ₦1K - ₦50K | 1.4% + ₦100 | 0.5% | ~1.9-2.0% |
| 2 | ₦50K - ₦500K | 1.4% (cap ₦2K) | 0.3% | ~1.7% |
| 3 | ₦500K+ | 1.4% (cap ₦2K) | 0.2% | ~0.4% |

### Offramp (Sell cNGN)

| Provider | Method | Fee | Platform Fee |
|----------|--------|-----|--------------|
| Flutterwave | Bank Transfer | 0.8% + ₦50 (cap ₦5K) | 0.5% |
| Paystack | Bank Transfer | ₦50 flat | 0.5% |

## 🧪 Testing

### Run All Tests

```bash
cargo test fee_calculation
```

### Run Specific Test

```bash
cargo test test_tier1_small_amount_fees
```

### Run Demo

```bash
cargo run --example fee_calculation_demo
```

## 🔧 Administration

### Add New Fee Structure

```sql
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method,
 min_amount, max_amount, provider_fee_percent, provider_fee_flat,
 provider_fee_cap, platform_fee_percent, is_active)
VALUES 
('onramp', 'new_provider', 'card', 
 1000, 50000, 1.5, 0, 2000, 0.5, true);
```

### Update Existing Fees

```sql
-- Deactivate old structure
UPDATE fee_structures 
SET is_active = FALSE, effective_until = NOW()
WHERE id = 'old-structure-id';

-- Insert new structure
INSERT INTO fee_structures (...) VALUES (...);
```

### Invalidate Cache (After Updates)

```rust
service.invalidate_cache().await;
```

## 📈 Monitoring Queries

### Daily Fee Revenue

```sql
SELECT 
    transaction_type,
    SUM(total_fees) as revenue,
    AVG(effective_rate) as avg_rate,
    COUNT(*) as count
FROM fee_calculation_logs
WHERE created_at >= CURRENT_DATE
GROUP BY transaction_type;
```

### Provider Comparison

```sql
SELECT 
    payment_provider,
    payment_method,
    AVG(provider_fee) as avg_fee,
    COUNT(*) as usage_count
FROM fee_calculation_logs
WHERE created_at >= CURRENT_DATE - INTERVAL '7 days'
GROUP BY payment_provider, payment_method
ORDER BY usage_count DESC;
```

### Fee Tier Distribution

```sql
SELECT 
    CASE 
        WHEN amount < 50000 THEN 'Tier 1 (< 50K)'
        WHEN amount < 500000 THEN 'Tier 2 (50K-500K)'
        ELSE 'Tier 3 (> 500K)'
    END as tier,
    COUNT(*) as count,
    AVG(effective_rate) as avg_rate,
    SUM(total_fees) as total_revenue
FROM fee_calculation_logs
WHERE transaction_type = 'onramp'
  AND created_at >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY tier
ORDER BY tier;
```

## 📁 Files Created

### Core Implementation
- `src/services/fee_calculation.rs` - Main service
- `migrations/20260220000000_enhanced_fee_structures.sql` - Database schema
- `db/seed_fee_structures.sql` - Initial fee data

### Testing & Examples
- `tests/fee_calculation_test.rs` - Comprehensive tests
- `examples/fee_calculation_demo.rs` - Interactive demo

### Documentation
- `docs/FEE_CALCULATION.md` - Full documentation
- `FEE_CALCULATION_IMPLEMENTATION.md` - Implementation summary
- `QUICK_START_FEE_CALCULATION.md` - This guide

### Scripts
- `setup-fee-structures.sh` - Automated setup

## ✨ Key Features

- ✅ Tiered fee structure (3 tiers)
- ✅ Provider-specific fees (Flutterwave, Paystack)
- ✅ Payment method support (card, bank transfer, USSD)
- ✅ Fee caps for user protection
- ✅ Stellar network fees (absorbed)
- ✅ In-memory caching for performance
- ✅ Audit logging for all calculations
- ✅ Fee estimation without provider
- ✅ Database-driven configuration
- ✅ Comprehensive test coverage

## 🎯 Real-World Examples

### Small Transaction (₦10,000)
```
Provider fee: ₦240 (1.4% + ₦100)
Platform fee: ₦50 (0.5%)
Total: ₦290 (2.9%)
You receive: 9,710 cNGN
```

### Medium Transaction (₦100,000)
```
Provider fee: ₦1,400 (1.4%)
Platform fee: ₦300 (0.3%)
Total: ₦1,700 (1.7%)
You receive: 98,300 cNGN
```

### Large Transaction (₦1,000,000)
```
Provider fee: ₦2,000 (capped)
Platform fee: ₦2,000 (0.2%)
Total: ₦4,000 (0.4%)
You receive: 996,000 cNGN
```

## 🔍 Troubleshooting

### No matching tier found
- Verify fee structures exist for the transaction type
- Check if amount is within configured ranges
- Ensure structures are active

### Incorrect calculations
- Review fee structure configuration
- Check for overlapping tiers
- Verify decimal precision

### Cache issues
- Invalidate cache after updates
- Check cache TTL settings

## 📚 Additional Resources

- Full Documentation: `docs/FEE_CALCULATION.md`
- Implementation Details: `FEE_CALCULATION_IMPLEMENTATION.md`
- Database Schema: `migrations/20260220000000_enhanced_fee_structures.sql`
- Test Examples: `tests/fee_calculation_test.rs`

## 🎉 Success!

The fee calculation service is production-ready and fully tested. It provides:
- Accurate, transparent fee calculations
- Flexible, database-driven configuration
- High performance with caching
- Complete audit trail
- Easy administration

**Ready to integrate with quote endpoints and power the Aframp platform!**
