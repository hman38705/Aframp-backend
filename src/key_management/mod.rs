//! Platform Key Management Framework
//!
//! Unified lifecycle management for all cryptographic keys:
//! - JWT signing (RS256, 90-day rotation)
//! - Payload encryption (ECDH-ES+A256KW, 180-day rotation)
//! - DB field encryption (AES-256-GCM, 365-day rotation)
//! - HMAC derivation (HMAC-SHA256, 90-day rotation)
//! - Backup encryption (AES-256-GCM, 365-day rotation)
//!
//! Key material is NEVER stored in the database — only metadata.
//! All material lives in the secrets manager (env vars in dev).

pub mod catalogue;
pub mod emergency;
pub mod escrow;
pub mod metrics;
pub mod provider;
pub mod reencryption;
pub mod rotation;

#[cfg(test)]
pub mod tests;

// Re-export the most commonly needed types for convenience.
pub use provider::{
    key_provider_from_env, AwsSecretsManagerKeyProvider, EnvKeyProvider, KeyProvider,
    KeyProviderError, KeyProviderKind, VaultKeyProvider, ZeroizingKey,
};
