# Fee Calculation Service

## Overview

The Fee Calculation Service provides dynamic, tiered fee calculations for all Aframp transactions. It supports provider-specific fees, amount-based tiers, and transparent fee breakdowns.

## Features

- **Tiered Fee Structure**: Lower fees for larger transactions
- **Provider-Specific Fees**: Different rates for Flutterwave, Paystack, etc.
- **Payment Method Support**: Card, bank transfer, USSD, etc.
- **Fee Caps**: Maximum fee limits to protect users on large transactions
- **Stellar Network Fees**: Included but absorbed by platform
- **Caching**: In-memory cache for performance
- **Audit Logging**: All calculations logged for transparency

## Usage

### Basic Fee Calculation

```rust
use aframp_backend::services::fee_calculation::FeeCalculationService;
use bigdecimal::BigDecimal;

let service = FeeCalculationService::new(pool);

let breakdown = service.calculate_fees(
    "onramp",                    // transaction type
    BigDecimal::from(100000),    // amount in NGN
    Some("flutterwave"),         // payment provider
    Some("card"),                // payment method
).await?;

println!("Total fees: ₦{}", breakdown.total);
println!("Net amount: ₦{}", breakdown.net_amount);
println!("Effective rate: {}%", breakdown.effective_rate);
```

### Fee Estimation

Get fee range without specifying provider:

```rust
let (min_fee, max_fee) = service.estimate_fees(
    "onramp",
    BigDecimal::from(100000),
).await?;

println!("Fees range from ₦{} to ₦{}", min_fee, max_fee);
```

### Fee Breakdown Structure

```json
{
  "amount": "100000.00",
  "currency": "NGN",
  "provider": {
    "name": "flutterwave",
    "method": "card",
    "percent": "1.4",
    "flat": "0",
    "cap": "2000",
    "calculated": "1400.00"
  },
  "platform": {
    "percent": "0.3",
    "calculated": "300.00"
  },
  "stellar": {
    "xlm": "0.00001",
    "ngn": "0.00",
    "absorbed": true
  },
  "total": "1700.00",
  "net_amount": "98300.00",
  "effective_rate": "1.7"
}
```

## Fee Tiers

### Onramp (Buy cNGN)

#### Tier 1: ₦1,000 - ₦50,000
- **Flutterwave Card**: 1.4% + ₦100 flat (capped at ₦2,000)
- **Paystack Card**: 1.5% (capped at ₦2,000)
- **Platform Fee**: 0.5%
- **Effective Rate**: ~1.9-2.0%

#### Tier 2: ₦50,001 - ₦500,000
- **Flutterwave Card**: 1.4% (capped at ₦2,000)
- **Paystack Card**: 1.5% (capped at ₦2,000)
- **Platform Fee**: 0.3%
- **Effective Rate**: ~1.7-1.8%

#### Tier 3: ₦500,001+
- **Flutterwave Card**: 1.4% (capped at ₦2,000)
- **Paystack Card**: 1.5% (capped at ₦2,000)
- **Platform Fee**: 0.2%
- **Effective Rate**: ~0.4-0.6% (due to cap)

### Offramp (Sell cNGN)

- **Flutterwave Bank Transfer**: 0.8% + ₦50 (min ₦50, max ₦5,000)
- **Paystack Bank Transfer**: ₦50 flat
- **Platform Fee**: 0.5% (small), 0.3% (medium), 0.2% (large)

### Bill Payment

- **Provider Fee**: 0.5% + ₦50 convenience fee (capped at ₦1,000)
- **Platform Fee**: 0.1%

## Examples

### Example 1: Small Onramp (₦10,000)

```
Amount: ₦10,000
Provider: Flutterwave Card

Provider fee: ₦10,000 × 1.4% + ₦100 = ₦240
Platform fee: ₦10,000 × 0.5% = ₦50
Stellar fee: ₦0 (absorbed)
─────────────────────────────────────
Total fees: ₦290
You receive: 9,710 cNGN
Effective rate: 2.9%
```

### Example 2: Large Onramp (₦1,000,000)

```
Amount: ₦1,000,000
Provider: Flutterwave Card

Provider fee: ₦1,000,000 × 1.4% = ₦14,000
  → Capped at ₦2,000
Platform fee: ₦1,000,000 × 0.2% = ₦2,000
Stellar fee: ₦0 (absorbed)
─────────────────────────────────────
Total fees: ₦4,000
You receive: 996,000 cNGN
Effective rate: 0.4%
```

### Example 3: Offramp (₦100,000)

```
Amount: ₦100,000 cNGN
Provider: Flutterwave Bank Transfer

Provider fee: ₦100,000 × 0.8% = ₦800
Platform fee: ₦100,000 × 0.5% = ₦500
─────────────────────────────────────
Total fees: ₦1,300
You receive: ₦98,700 NGN
Effective rate: 1.3%
```

## Database Schema

### fee_structures Table

```sql
CREATE TABLE fee_structures (
    id UUID PRIMARY KEY,
    transaction_type TEXT NOT NULL,
    payment_provider TEXT,
    payment_method TEXT,
    min_amount NUMERIC(36, 18),
    max_amount NUMERIC(36, 18),
    provider_fee_percent NUMERIC(10, 4),
    provider_fee_flat NUMERIC(36, 18),
    provider_fee_cap NUMERIC(36, 18),
    platform_fee_percent NUMERIC(10, 4),
    platform_fee_flat NUMERIC(36, 18),
    is_active BOOLEAN DEFAULT TRUE,
    effective_from TIMESTAMPTZ DEFAULT NOW(),
    effective_until TIMESTAMPTZ,
    metadata JSONB DEFAULT '{}'
);
```

### fee_calculation_logs Table

```sql
CREATE TABLE fee_calculation_logs (
    id UUID PRIMARY KEY,
    transaction_id UUID,
    transaction_type TEXT NOT NULL,
    amount NUMERIC(36, 18) NOT NULL,
    currency TEXT NOT NULL,
    payment_provider TEXT,
    payment_method TEXT,
    fee_structure_id UUID,
    provider_fee NUMERIC(36, 18),
    platform_fee NUMERIC(36, 18),
    stellar_fee_xlm NUMERIC(36, 18),
    stellar_fee_ngn NUMERIC(36, 18),
    total_fees NUMERIC(36, 18),
    net_amount NUMERIC(36, 18),
    effective_rate NUMERIC(10, 4),
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

## Administration

### Adding New Fee Structure

```sql
INSERT INTO fee_structures 
(transaction_type, payment_provider, payment_method, 
 min_amount, max_amount, provider_fee_percent, provider_fee_flat,
 provider_fee_cap, platform_fee_percent, is_active)
VALUES 
('onramp', 'flutterwave', 'card', 
 1000, 50000, 1.4, 100, 2000, 0.5, true);
```

### Updating Fee Structure

To update fees, create a new structure and deactivate the old one:

```sql
-- Deactivate old structure
UPDATE fee_structures 
SET is_active = FALSE, effective_until = NOW()
WHERE id = 'old-structure-id';

-- Insert new structure
INSERT INTO fee_structures (...) VALUES (...);
```

### Invalidating Cache

After updating fee structures, invalidate the cache:

```rust
service.invalidate_cache().await;
```

## Monitoring

### Key Metrics

- Average effective rate by tier
- Fee revenue per transaction type
- Provider fee comparison
- Cache hit rate
- Fee calculation errors

### Audit Queries

```sql
-- Total fees collected today
SELECT 
    transaction_type,
    SUM(total_fees) as total_fees,
    AVG(effective_rate) as avg_rate,
    COUNT(*) as transaction_count
FROM fee_calculation_logs
WHERE created_at >= CURRENT_DATE
GROUP BY transaction_type;

-- Fee breakdown by provider
SELECT 
    payment_provider,
    payment_method,
    AVG(provider_fee) as avg_provider_fee,
    AVG(platform_fee) as avg_platform_fee,
    COUNT(*) as count
FROM fee_calculation_logs
WHERE created_at >= CURRENT_DATE - INTERVAL '7 days'
GROUP BY payment_provider, payment_method;
```

## Performance

- **Caching**: Fee structures cached in memory for 1 hour
- **Cache Invalidation**: Manual or on update
- **XLM Rate Caching**: Updated every 5 minutes
- **Database Indexes**: Optimized for tier lookup

## Testing

Run tests:

```bash
cargo test fee_calculation
```

Run specific test:

```bash
cargo test test_tier1_small_amount_fees
```

## Setup

1. Run migration:
```bash
sqlx migrate run
```

2. Seed fee structures:
```bash
./setup-fee-structures.sh
```

3. Verify setup:
```bash
psql $DATABASE_URL -c "SELECT COUNT(*) FROM fee_structures WHERE is_active = true;"
```

## Best Practices

1. **Always use BigDecimal** for money calculations
2. **Log all calculations** for audit trail
3. **Cache aggressively** - fees don't change often
4. **Test tier boundaries** thoroughly
5. **Monitor effective rates** to stay competitive
6. **Update fees during low-traffic periods**
7. **Keep historical fee structures** for disputes

## Troubleshooting

### No matching tier found

- Check if fee structures exist for transaction type
- Verify amount is within configured ranges
- Check if structures are active

### Incorrect fee calculation

- Review fee structure configuration
- Check for overlapping tiers
- Verify decimal precision

### Cache issues

- Invalidate cache after updates
- Check cache TTL settings
- Monitor cache hit rate

## Future Enhancements

- [ ] Volume-based discounts
- [ ] Dynamic fee adjustment based on demand
- [ ] A/B testing for fee structures
- [ ] Real-time competitor fee monitoring
- [ ] Automated fee optimization
- [ ] User-specific fee tiers (VIP)

## Support

For issues or questions:
- Check logs: `fee_calculation_logs` table
- Review fee structures: `fee_structures` table
- Contact: backend-team@aframp.com
