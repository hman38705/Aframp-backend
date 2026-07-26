//! Type aliases for the historical `chains::stellar::types` names, mapped
//! onto the compatibility shim's structs in [`super::client`].

pub type AssetBalance = super::client::StellarBalance;
pub type StellarAccountInfo = super::client::StellarAccount;

/// Basic Stellar public-key address format check: `G` prefix, 56 chars,
/// base32 alphabet (A-Z, 2-7). Does not verify the ed25519 checksum.
pub fn is_valid_stellar_address(address: &str) -> bool {
    address.len() == 56
        && address.starts_with('G')
        && address
            .chars()
            .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
}

/// Find `asset_code` "cNGN" (case-insensitive) in `balances`, optionally
/// scoped to `issuer`, and parse its balance as `f64`. Returns `0.0` if no
/// matching trustline is found.
pub fn extract_cngn_balance(balances: &[AssetBalance], issuer: Option<&str>) -> f64 {
    balances
        .iter()
        .find(|b| {
            b.asset_code.as_deref().map(|c| c.eq_ignore_ascii_case("cNGN")) == Some(true)
                && issuer.map_or(true, |i| b.asset_issuer.as_deref() == Some(i))
        })
        .and_then(|b| b.balance.parse::<f64>().ok())
        .unwrap_or(0.0)
}
