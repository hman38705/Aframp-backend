//! Property-based tests for KYC transaction limit enforcement (issue #797).
//!
//! `TransactionLimitEnforcer::check_transaction_limits` is pure (no DB/redis
//! access), so it can be exercised directly with proptest-generated inputs.

use aframp_backend::database::kyc_repository::KycTier;
use aframp_backend::kyc::tier_requirements::TransactionLimitEnforcer;
use bigdecimal::BigDecimal;
use proptest::prelude::*;

fn cents_to_decimal(cents: i64) -> BigDecimal {
    BigDecimal::from(cents) / BigDecimal::from(100)
}

fn any_tier() -> impl Strategy<Value = KycTier> {
    prop_oneof![
        Just(KycTier::Unverified),
        Just(KycTier::Basic),
        Just(KycTier::Standard),
        Just(KycTier::Enhanced),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// If a transaction is allowed, adding it to the current daily and
    /// monthly volumes must never push either over its limit, and the
    /// transaction itself must not exceed the single-transaction cap.
    #[test]
    fn allowed_transaction_never_exceeds_limits(
        tier in any_tier(),
        amount_cents in 1i64..10_000_000_00i64,
        daily_used_cents in 0i64..10_000_000_00i64,
        monthly_used_cents in 0i64..50_000_000_00i64,
    ) {
        let amount = cents_to_decimal(amount_cents);
        let daily_used = cents_to_decimal(daily_used_cents);
        let monthly_used = cents_to_decimal(monthly_used_cents);

        let enforcer = TransactionLimitEnforcer::new(tier.clone());
        let result = enforcer.check_transaction_limits(
            amount.clone(),
            daily_used.clone(),
            monthly_used.clone(),
        );

        if result.is_allowed {
            let limits = aframp_backend::kyc::tier_requirements::KycTierRequirements::get_tier_limits(tier);
            prop_assert!(amount.clone() <= limits.max_transaction_amount);
            prop_assert!(daily_used + amount.clone() <= limits.daily_volume_limit);
            prop_assert!(monthly_used + amount <= limits.monthly_volume_limit);
        }
    }

    /// If adding the transaction would push the daily (or monthly) volume
    /// over its limit, the transaction must be rejected — the enforcer
    /// must never silently allow a window to exceed its configured cap.
    #[test]
    fn over_limit_transaction_is_never_allowed(
        tier in any_tier(),
        amount_cents in 1i64..10_000_000_00i64,
        daily_used_cents in 0i64..10_000_000_00i64,
        monthly_used_cents in 0i64..50_000_000_00i64,
    ) {
        let amount = cents_to_decimal(amount_cents);
        let daily_used = cents_to_decimal(daily_used_cents);
        let monthly_used = cents_to_decimal(monthly_used_cents);

        let limits = aframp_backend::kyc::tier_requirements::KycTierRequirements::get_tier_limits(tier.clone());
        let would_exceed_daily = daily_used.clone() + amount.clone() > limits.daily_volume_limit;
        let would_exceed_monthly = monthly_used.clone() + amount.clone() > limits.monthly_volume_limit;
        let would_exceed_single = amount.clone() > limits.max_transaction_amount;

        let enforcer = TransactionLimitEnforcer::new(tier);
        let result = enforcer.check_transaction_limits(amount, daily_used, monthly_used);

        if would_exceed_daily || would_exceed_monthly || would_exceed_single {
            prop_assert!(!result.is_allowed);
            prop_assert!(!result.violations.is_empty());
        }
    }
}
