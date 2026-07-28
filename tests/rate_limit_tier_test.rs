//! Tests for endpoint sensitivity tiers (Issue #726)
//!
//! Verifies that CRITICAL / FINANCIAL / STANDARD / PUBLIC tiers defined in
//! rate_limits.yaml resolve to the correct per-IP limit for representative
//! routes, and that an endpoint's explicit `per_ip` still wins over its tier.

use aframp_backend::middleware::rate_limit::{EndpointTier, RateLimitConfig};

fn load_config() -> RateLimitConfig {
    RateLimitConfig::load("rate_limits.yaml").expect("rate_limits.yaml should load")
}

#[test]
fn critical_tier_applies_to_mint_and_redemption() {
    let config = load_config();

    for path in ["/api/mint/requests", "/api/redemption/initiate"] {
        let limits = config.get_limits(path);
        assert_eq!(limits.tier, Some(EndpointTier::Critical), "path: {path}");
        let per_ip = limits.per_ip.expect("critical tier must resolve a per_ip limit");
        assert_eq!(per_ip.limit, 10);
        assert_eq!(per_ip.window, 60);
    }
}

#[test]
fn financial_tier_applies_to_offramp_and_onramp_initiate() {
    let config = load_config();

    for path in ["/api/onramp/initiate", "/api/offramp/initiate"] {
        let limits = config.get_limits(path);
        assert_eq!(limits.tier, Some(EndpointTier::Financial), "path: {path}");
        let per_ip = limits.per_ip.expect("financial tier must resolve a per_ip limit");
        assert_eq!(per_ip.limit, 60);
        assert_eq!(per_ip.window, 60);
    }
}

#[test]
fn standard_tier_applies_to_default_fallback() {
    let config = load_config();

    let limits = config.get_limits("/api/some/unlisted/route");
    assert_eq!(limits.tier, Some(EndpointTier::Standard));
    let per_ip = limits.per_ip.expect("default entry defines per_ip explicitly");
    assert_eq!(per_ip.limit, 100);
    assert_eq!(per_ip.window, 60);
}

#[test]
fn public_tier_applies_to_rates_endpoint() {
    let config = load_config();

    let limits = config.get_limits("/api/rates");
    assert_eq!(limits.tier, Some(EndpointTier::Public));
    let per_ip = limits.per_ip.expect("public tier must resolve a per_ip limit");
    assert_eq!(per_ip.limit, 100, "explicit per_ip on /api/rates overrides tier default");
}

#[test]
fn tier_default_used_when_endpoint_omits_per_ip() {
    let config = load_config();

    // /api/wallet/balance only sets per_wallet + tier: STANDARD, no per_ip.
    let limits = config.get_limits("/api/wallet/balance");
    assert_eq!(limits.tier, Some(EndpointTier::Standard));
    let per_ip = limits.per_ip.expect("tier fallback should populate per_ip");
    assert_eq!(per_ip.limit, 300);
    assert_eq!(per_ip.window, 60);
}
