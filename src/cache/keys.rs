//! Type-safe cache key builders
//!
//! Provides structured key generation to prevent collisions and ensure consistency.
//! All keys follow the pattern: v1:{namespace}:{identifier}

use std::fmt;

/// Cache key namespace constants
pub const VERSION: &str = "v1";

/// Wallet-related cache keys
pub mod wallet {
    use super::*;

    pub const NAMESPACE: &str = "wallet";

    /// Wallet balance key: v1:wallet:balance:{address}
    #[derive(Debug, Clone)]
    pub struct BalanceKey {
        pub address: String,
    }

    impl BalanceKey {
        pub fn new(address: impl Into<String>) -> Self {
            Self {
                address: address.into(),
            }
        }
    }

    impl fmt::Display for BalanceKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:balance:{}", VERSION, NAMESPACE, self.address)
        }
    }

    /// Trustline existence key: v1:wallet:trustline:{address}
    #[derive(Debug, Clone)]
    pub struct TrustlineKey {
        pub address: String,
    }

    impl TrustlineKey {
        pub fn new(address: impl Into<String>) -> Self {
            Self {
                address: address.into(),
            }
        }
    }

    impl fmt::Display for TrustlineKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:trustline:{}", VERSION, NAMESPACE, self.address)
        }
    }

    /// Transaction count key: v1:wallet:tx_count:{address}
    #[derive(Debug, Clone)]
    pub struct TransactionCountKey {
        pub address: String,
    }

    impl TransactionCountKey {
        pub fn new(address: impl Into<String>) -> Self {
            Self {
                address: address.into(),
            }
        }
    }

    impl fmt::Display for TransactionCountKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:tx_count:{}", VERSION, NAMESPACE, self.address)
        }
    }
}

/// Exchange rate cache keys
pub mod exchange_rate {
    use super::*;

    pub const NAMESPACE: &str = "rate";

    /// Currency pair rate key: v1:rate:{from_currency}:{to_currency}
    #[derive(Debug, Clone)]
    pub struct CurrencyPairKey {
        pub from_currency: String,
        pub to_currency: String,
    }

    impl CurrencyPairKey {
        pub fn new(from_currency: impl Into<String>, to_currency: impl Into<String>) -> Self {
            Self {
                from_currency: from_currency.into(),
                to_currency: to_currency.into(),
            }
        }

        /// Create AFRI-specific rate key
        pub fn afri_rate(to_currency: impl Into<String>) -> Self {
            Self::new("AFRI", to_currency)
        }
    }

    impl fmt::Display for CurrencyPairKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:{}:{}", VERSION, NAMESPACE, self.from_currency, self.to_currency)
        }
    }

    /// Conversion calculation key: v1:rate:convert:{amount}:{from}:{to}
    #[derive(Debug, Clone)]
    pub struct ConversionKey {
        pub amount: String,
        pub from_currency: String,
        pub to_currency: String,
    }

    impl ConversionKey {
        pub fn new(amount: impl Into<String>, from_currency: impl Into<String>, to_currency: impl Into<String>) -> Self {
            Self {
                amount: amount.into(),
                from_currency: from_currency.into(),
                to_currency: to_currency.into(),
            }
        }
    }

    impl fmt::Display for ConversionKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:convert:{}:{}:{}", VERSION, NAMESPACE, self.amount, self.from_currency, self.to_currency)
        }
    }
}

/// Transaction cache keys
pub mod transaction {
    use super::*;

    pub const NAMESPACE: &str = "transaction";

    /// Transaction status key: v1:transaction:status:{tx_hash}
    #[derive(Debug, Clone)]
    pub struct StatusKey {
        pub tx_hash: String,
    }

    impl StatusKey {
        pub fn new(tx_hash: impl Into<String>) -> Self {
            Self {
                tx_hash: tx_hash.into(),
            }
        }
    }

    impl fmt::Display for StatusKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:status:{}", VERSION, NAMESPACE, self.tx_hash)
        }
    }

    /// Recent transactions key: v1:transaction:recent:{address}
    #[derive(Debug, Clone)]
    pub struct RecentKey {
        pub address: String,
    }

    impl RecentKey {
        pub fn new(address: impl Into<String>) -> Self {
            Self {
                address: address.into(),
            }
        }
    }

    impl fmt::Display for RecentKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:recent:{}", VERSION, NAMESPACE, self.address)
        }
    }
}

/// Authentication and session cache keys
pub mod auth {
    use super::*;

    pub const NAMESPACE: &str = "auth";

    /// User session key: v1:auth:session:{session_id}
    #[derive(Debug, Clone)]
    pub struct SessionKey {
        pub session_id: String,
    }

    impl SessionKey {
        pub fn new(session_id: impl Into<String>) -> Self {
            Self {
                session_id: session_id.into(),
            }
        }
    }

    impl fmt::Display for SessionKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:session:{}", VERSION, NAMESPACE, self.session_id)
        }
    }

    /// JWT validation key: v1:auth:jwt:{token_hash}
    #[derive(Debug, Clone)]
    pub struct JwtKey {
        pub token_hash: String,
    }

    impl JwtKey {
        pub fn new(token_hash: impl Into<String>) -> Self {
            Self {
                token_hash: token_hash.into(),
            }
        }
    }

    impl fmt::Display for JwtKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:jwt:{}", VERSION, NAMESPACE, self.token_hash)
        }
    }

    /// Rate limiting key: v1:auth:rate_limit:{identifier}:{action}
    #[derive(Debug, Clone)]
    pub struct RateLimitKey {
        pub identifier: String,
        pub action: String,
    }

    impl RateLimitKey {
        pub fn new(identifier: impl Into<String>, action: impl Into<String>) -> Self {
            Self {
                identifier: identifier.into(),
                action: action.into(),
            }
        }
    }

    impl fmt::Display for RateLimitKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:rate_limit:{}:{}", VERSION, NAMESPACE, self.identifier, self.action)
        }
    }
}

/// Bill payment cache keys
pub mod bill_payment {
    use super::*;

    pub const NAMESPACE: &str = "bill";

    /// Provider configuration key: v1:bill:provider:{provider_id}
    #[derive(Debug, Clone)]
    pub struct ProviderKey {
        pub provider_id: String,
    }

    impl ProviderKey {
        pub fn new(provider_id: impl Into<String>) -> Self {
            Self {
                provider_id: provider_id.into(),
            }
        }
    }

    impl fmt::Display for ProviderKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:provider:{}", VERSION, NAMESPACE, self.provider_id)
        }
    }

    /// Provider availability key: v1:bill:available:{country_code}
    #[derive(Debug, Clone)]
    pub struct AvailabilityKey {
        pub country_code: String,
    }

    impl AvailabilityKey {
        pub fn new(country_code: impl Into<String>) -> Self {
            Self {
                country_code: country_code.into(),
            }
        }
    }

    impl fmt::Display for AvailabilityKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:available:{}", VERSION, NAMESPACE, self.country_code)
        }
    }
}

/// Fee structure cache keys
pub mod fee {
    use super::*;

    pub const NAMESPACE: &str = "fee";

    /// Fee structure key: v1:fee:structure:{fee_type}
    #[derive(Debug, Clone)]
    pub struct StructureKey {
        pub fee_type: String,
    }

    impl StructureKey {
        pub fn new(fee_type: impl Into<String>) -> Self {
            Self {
                fee_type: fee_type.into(),
            }
        }
    }

    impl fmt::Display for StructureKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}:{}:structure:{}", VERSION, NAMESPACE, self.fee_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_balance_key() {
        let key = wallet::BalanceKey::new("GA123456789");
        assert_eq!(key.to_string(), "v1:wallet:balance:GA123456789");
    }

    #[test]
    fn test_exchange_rate_key() {
        let key = exchange_rate::CurrencyPairKey::afri_rate("USD");
        assert_eq!(key.to_string(), "v1:rate:AFRI:USD");
    }

    #[test]
    fn test_conversion_key() {
        let key = exchange_rate::ConversionKey::new("100.50", "AFRI", "USD");
        assert_eq!(key.to_string(), "v1:rate:convert:100.50:AFRI:USD");
    }

    #[test]
    fn test_session_key() {
        let key = auth::SessionKey::new("session_123");
        assert_eq!(key.to_string(), "v1:auth:session:session_123");
    }

    #[test]
    fn test_rate_limit_key() {
        let key = auth::RateLimitKey::new("user_123", "login");
        assert_eq!(key.to_string(), "v1:auth:rate_limit:user_123:login");
    }
}