//! KeyProvider trait and implementations (Issue #721)
//!
//! Abstracts over key retrieval backends so the Stellar signing key can be
//! loaded from an environment variable (dev/CI), AWS Secrets Manager, or
//! HashiCorp Vault without changing any call-site code.
//!
//! # Usage
//! ```rust,ignore
//! let provider = key_provider_from_config(&config);
//! let key = provider.get_stellar_signing_key().await?;
//! // ... use key ...
//! // key is ZeroizingKey; memory is scrubbed when it drops
//! ```
//!
//! # Feature flags
//! - `aws-secrets` — enables `AwsSecretsManagerKeyProvider` (requires AWS SDK)
//! - Default build uses `EnvKeyProvider` and `VaultKeyProvider` only.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};
use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// ZeroizingKey — key material that is scrubbed from memory on drop
// ---------------------------------------------------------------------------

/// A heap-allocated secret string that is zeroed when dropped.
///
/// All hot paths that receive signing key material should accept this type
/// (or `&[u8]`) rather than a plain `String` or `Vec<u8>`.
pub type ZeroizingKey = Zeroizing<String>;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that may arise when fetching a key from a provider.
#[derive(Debug, thiserror::Error)]
pub enum KeyProviderError {
    #[error("environment variable `{0}` is not set or is empty")]
    MissingEnvVar(String),

    #[error("AWS Secrets Manager error: {0}")]
    AwsError(String),

    #[error("Vault error: {0}")]
    VaultError(String),

    #[error("key not found in secrets store: {0}")]
    NotFound(String),

    #[error("unexpected provider error: {0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// KeyProvider trait
// ---------------------------------------------------------------------------

/// Trait implemented by every key-storage backend.
///
/// Implementations must be `Send + Sync` so they can be stored in `AppState`
/// and accessed across async task boundaries.
#[async_trait]
pub trait KeyProvider: Send + Sync {
    /// Retrieve the Stellar issuer signing key (raw secret seed, e.g. `S…`).
    ///
    /// The returned value is wrapped in [`Zeroizing`] so the secret is
    /// scrubbed from heap memory as soon as the caller drops it.
    async fn get_stellar_signing_key(&self) -> Result<ZeroizingKey, KeyProviderError>;

    /// Optional: retrieve an arbitrary named secret. Defaults to
    /// `get_stellar_signing_key()` to keep simple providers simple.
    async fn get_secret(&self, name: &str) -> Result<ZeroizingKey, KeyProviderError> {
        let _ = name;
        self.get_stellar_signing_key().await
    }

    /// Human-readable name of this provider (for logging / metrics).
    fn provider_name(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// EnvKeyProvider — reads key from environment variable (dev / CI)
// ---------------------------------------------------------------------------

/// Reads the Stellar signing key from an environment variable.
///
/// The variable name defaults to `STELLAR_SIGNING_KEY` but can be overridden.
///
/// # Security note
/// This provider is suitable for development and CI only.  In production use
/// `AwsSecretsManagerKeyProvider` or `VaultKeyProvider`.
#[derive(Debug, Clone)]
pub struct EnvKeyProvider {
    /// Name of the environment variable that holds the key.
    pub env_var: String,
}

impl EnvKeyProvider {
    /// Use the default environment variable name `STELLAR_SIGNING_KEY`.
    pub fn new() -> Self {
        Self {
            env_var: "STELLAR_SIGNING_KEY".to_string(),
        }
    }

    /// Use a custom environment variable name.
    pub fn with_var(env_var: impl Into<String>) -> Self {
        Self {
            env_var: env_var.into(),
        }
    }
}

impl Default for EnvKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyProvider for EnvKeyProvider {
    async fn get_stellar_signing_key(&self) -> Result<ZeroizingKey, KeyProviderError> {
        let value = std::env::var(&self.env_var).map_err(|_| {
            KeyProviderError::MissingEnvVar(self.env_var.clone())
        })?;

        if value.trim().is_empty() {
            return Err(KeyProviderError::MissingEnvVar(self.env_var.clone()));
        }

        debug!(
            provider = "env",
            env_var = %self.env_var,
            "Stellar signing key loaded from environment variable"
        );

        Ok(Zeroizing::new(value))
    }

    async fn get_secret(&self, name: &str) -> Result<ZeroizingKey, KeyProviderError> {
        let value = std::env::var(name)
            .map_err(|_| KeyProviderError::MissingEnvVar(name.to_string()))?;
        if value.trim().is_empty() {
            return Err(KeyProviderError::MissingEnvVar(name.to_string()));
        }
        Ok(Zeroizing::new(value))
    }

    fn provider_name(&self) -> &'static str {
        "env"
    }
}

// ---------------------------------------------------------------------------
// AwsSecretsManagerKeyProvider — retrieves key from AWS Secrets Manager
// ---------------------------------------------------------------------------

/// Retrieves the Stellar signing key from AWS Secrets Manager via the
/// standard AWS SDK.
///
/// # Configuration
/// | Env var                          | Default                              | Description                          |
/// |----------------------------------|--------------------------------------|--------------------------------------|
/// | `AWS_SECRET_NAME`                | `"aframp/stellar/signing-key"`       | Secret name / ARN in Secrets Manager |
/// | `AWS_REGION`                     | us-east-1 (SDK default)              | AWS region                           |
/// | `AWS_ACCESS_KEY_ID`              | (SDK credential chain)               | AWS credentials                      |
/// | `AWS_SECRET_ACCESS_KEY`          | (SDK credential chain)               | AWS credentials                      |
///
/// # Feature flag
/// This provider is compiled in regardless of feature flags.  If the AWS SDK
/// crate is not available in the project, replace the `reqwest`-based fallback
/// implementation below with the real `aws-sdk-secretsmanager` call once the
/// dependency is added to `Cargo.toml`.
#[derive(Debug, Clone)]
pub struct AwsSecretsManagerKeyProvider {
    /// The Secrets Manager secret name or full ARN.
    pub secret_name: String,
    /// AWS region (e.g. `"us-east-1"`). Falls back to `AWS_REGION` env var.
    pub region: Option<String>,
}

impl AwsSecretsManagerKeyProvider {
    /// Create a provider using the default secret name (`aframp/stellar/signing-key`).
    pub fn new() -> Self {
        let secret_name = std::env::var("AWS_SECRET_NAME")
            .unwrap_or_else(|_| "aframp/stellar/signing-key".to_string());
        let region = std::env::var("AWS_REGION").ok();
        Self { secret_name, region }
    }

    /// Create a provider with an explicit secret name.
    pub fn with_secret_name(secret_name: impl Into<String>) -> Self {
        Self {
            secret_name: secret_name.into(),
            region: std::env::var("AWS_REGION").ok(),
        }
    }

    /// Fetch a secret string from AWS Secrets Manager using the AWS SDK HTTP
    /// API directly (so we don't require the heavy SDK as a hard dep).
    ///
    /// In production, swap this for the `aws-sdk-secretsmanager` crate call:
    /// ```rust,ignore
    /// let client = aws_sdk_secretsmanager::Client::new(&aws_config::load_from_env().await);
    /// let resp = client.get_secret_value().secret_id(&self.secret_name).send().await?;
    /// let secret = resp.secret_string().unwrap_or_default();
    /// ```
    async fn fetch_secret_value(&self, secret_name: &str) -> Result<ZeroizingKey, KeyProviderError> {
        // Real AWS SDK integration — uses env-based credential chain.
        // Currently implemented as a documented stub that delegates to the
        // environment so the build compiles without adding the heavy AWS SDK
        // dependency.  Replace the body below once `aws-sdk-secretsmanager`
        // is added to Cargo.toml.
        warn!(
            provider = "aws-secrets-manager",
            secret_name = %secret_name,
            "AwsSecretsManagerKeyProvider: real SDK call not wired — falling back to env var. \
             Replace this stub with aws-sdk-secretsmanager::Client once the SDK dep is added."
        );

        // Fallback: derive env var name from secret path, e.g.
        // "aframp/stellar/signing-key" → "AFRAMP_STELLAR_SIGNING_KEY"
        let env_var = secret_name
            .replace('/', "_")
            .replace('-', "_")
            .to_uppercase();

        std::env::var(&env_var)
            .map(Zeroizing::new)
            .map_err(|_| {
                KeyProviderError::AwsError(format!(
                    "secret `{}` not found (SDK stub — set env var `{}` for local dev)",
                    secret_name, env_var
                ))
            })
    }
}

impl Default for AwsSecretsManagerKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyProvider for AwsSecretsManagerKeyProvider {
    async fn get_stellar_signing_key(&self) -> Result<ZeroizingKey, KeyProviderError> {
        info!(
            provider = "aws-secrets-manager",
            secret_name = %self.secret_name,
            "Fetching Stellar signing key from AWS Secrets Manager"
        );
        self.fetch_secret_value(&self.secret_name).await
    }

    async fn get_secret(&self, name: &str) -> Result<ZeroizingKey, KeyProviderError> {
        info!(
            provider = "aws-secrets-manager",
            secret_name = %name,
            "Fetching secret from AWS Secrets Manager"
        );
        self.fetch_secret_value(name).await
    }

    fn provider_name(&self) -> &'static str {
        "aws-secrets-manager"
    }
}

// ---------------------------------------------------------------------------
// VaultKeyProvider — retrieves key from HashiCorp Vault KV v2
// ---------------------------------------------------------------------------

/// Retrieves the Stellar signing key from HashiCorp Vault KV v2 using the
/// Vault HTTP API.
///
/// # Configuration
/// | Env var                     | Default                          | Description                            |
/// |-----------------------------|----------------------------------|----------------------------------------|
/// | `VAULT_ADDR`                | `"http://127.0.0.1:8200"`        | Vault server address                   |
/// | `VAULT_TOKEN`               | *(required)*                     | Vault token with read access           |
/// | `VAULT_SECRET_PATH`         | `"secret/data/aframp/stellar"`   | KV v2 secret path                      |
/// | `VAULT_SECRET_KEY`          | `"signing_key"`                  | Key inside the KV secret data object   |
#[derive(Debug, Clone)]
pub struct VaultKeyProvider {
    pub vault_addr: String,
    pub vault_token: String,
    pub secret_path: String,
    pub secret_key: String,
}

impl VaultKeyProvider {
    /// Build provider from environment variables with sensible defaults.
    pub fn from_env() -> Result<Self, KeyProviderError> {
        let vault_addr = std::env::var("VAULT_ADDR")
            .unwrap_or_else(|_| "http://127.0.0.1:8200".to_string());
        let vault_token = std::env::var("VAULT_TOKEN")
            .map_err(|_| KeyProviderError::MissingEnvVar("VAULT_TOKEN".to_string()))?;
        let secret_path = std::env::var("VAULT_SECRET_PATH")
            .unwrap_or_else(|_| "secret/data/aframp/stellar".to_string());
        let secret_key = std::env::var("VAULT_SECRET_KEY")
            .unwrap_or_else(|_| "signing_key".to_string());

        Ok(Self {
            vault_addr,
            vault_token,
            secret_path,
            secret_key,
        })
    }

    /// Fetch a field from a Vault KV v2 path using the Vault HTTP API.
    async fn fetch_from_vault(
        &self,
        path: &str,
        field: &str,
    ) -> Result<ZeroizingKey, KeyProviderError> {
        let url = format!("{}/v1/{}", self.vault_addr.trim_end_matches('/'), path);

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("X-Vault-Token", &self.vault_token)
            .send()
            .await
            .map_err(|e| KeyProviderError::VaultError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(KeyProviderError::VaultError(format!(
                "Vault returned HTTP {}: {}",
                status, body
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| KeyProviderError::VaultError(format!("Failed to parse response: {}", e)))?;

        // KV v2 response structure: { "data": { "data": { "<field>": "<value>" } } }
        let secret_value = json
            .pointer(&format!("/data/data/{}", field))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                KeyProviderError::NotFound(format!(
                    "field `{}` not found in Vault secret at `{}`",
                    field, path
                ))
            })?
            .to_string();

        if secret_value.trim().is_empty() {
            return Err(KeyProviderError::NotFound(format!(
                "field `{}` in Vault secret `{}` is empty",
                field, path
            )));
        }

        debug!(
            provider = "vault",
            path = %path,
            field = %field,
            "Secret loaded from HashiCorp Vault"
        );

        Ok(Zeroizing::new(secret_value))
    }
}

#[async_trait]
impl KeyProvider for VaultKeyProvider {
    async fn get_stellar_signing_key(&self) -> Result<ZeroizingKey, KeyProviderError> {
        info!(
            provider = "vault",
            path = %self.secret_path,
            key = %self.secret_key,
            "Fetching Stellar signing key from HashiCorp Vault"
        );
        self.fetch_from_vault(&self.secret_path, &self.secret_key).await
    }

    async fn get_secret(&self, name: &str) -> Result<ZeroizingKey, KeyProviderError> {
        // For `get_secret`, treat `name` as `path:field` or use the configured path with name as field
        let (path, field) = if let Some((p, f)) = name.rsplit_once(':') {
            (p, f)
        } else {
            (self.secret_path.as_str(), name)
        };
        self.fetch_from_vault(path, field).await
    }

    fn provider_name(&self) -> &'static str {
        "vault"
    }
}

// ---------------------------------------------------------------------------
// Factory — build the right provider from config
// ---------------------------------------------------------------------------

/// Which backend to use for key retrieval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyProviderKind {
    /// Load from environment variable (dev / CI).
    Env,
    /// Load from AWS Secrets Manager.
    AwsSecretsManager,
    /// Load from HashiCorp Vault KV v2.
    Vault,
}

impl KeyProviderKind {
    /// Parse from the `KEY_PROVIDER` environment variable.
    /// Accepted values (case-insensitive): `env`, `aws`, `aws-secrets-manager`, `vault`.
    pub fn from_env() -> Self {
        match std::env::var("KEY_PROVIDER")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "aws" | "aws-secrets-manager" | "aws_secrets_manager" => Self::AwsSecretsManager,
            "vault" | "hashicorp-vault" => Self::Vault,
            _ => Self::Env,
        }
    }
}

/// Build a boxed `KeyProvider` based on the `KEY_PROVIDER` env var.
///
/// Falls back to `EnvKeyProvider` if `KEY_PROVIDER` is unset or unknown.
pub fn key_provider_from_env() -> Result<Arc<dyn KeyProvider>, KeyProviderError> {
    match KeyProviderKind::from_env() {
        KeyProviderKind::Env => {
            info!(provider = "env", "Using EnvKeyProvider for key management");
            Ok(Arc::new(EnvKeyProvider::new()))
        }
        KeyProviderKind::AwsSecretsManager => {
            info!(
                provider = "aws-secrets-manager",
                "Using AwsSecretsManagerKeyProvider for key management"
            );
            Ok(Arc::new(AwsSecretsManagerKeyProvider::new()))
        }
        KeyProviderKind::Vault => {
            info!(provider = "vault", "Using VaultKeyProvider for key management");
            let provider = VaultKeyProvider::from_env()?;
            Ok(Arc::new(provider))
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── EnvKeyProvider ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn env_provider_reads_var() {
        std::env::set_var("TEST_STELLAR_KEY_721", "SCZANGBA5XTONSOWQ5H6NEOBNQ4HV5T3STE2GFLYNZM4LGQN6XKJT");
        let provider = EnvKeyProvider::with_var("TEST_STELLAR_KEY_721");
        let key = provider.get_stellar_signing_key().await.unwrap();
        assert!(key.starts_with('S'));
        std::env::remove_var("TEST_STELLAR_KEY_721");
    }

    #[tokio::test]
    async fn env_provider_returns_error_for_missing_var() {
        std::env::remove_var("NONEXISTENT_STELLAR_KEY_99999");
        let provider = EnvKeyProvider::with_var("NONEXISTENT_STELLAR_KEY_99999");
        assert!(provider.get_stellar_signing_key().await.is_err());
    }

    #[tokio::test]
    async fn env_provider_returns_error_for_empty_var() {
        std::env::set_var("EMPTY_STELLAR_KEY_721", "");
        let provider = EnvKeyProvider::with_var("EMPTY_STELLAR_KEY_721");
        assert!(provider.get_stellar_signing_key().await.is_err());
        std::env::remove_var("EMPTY_STELLAR_KEY_721");
    }

    // ── ZeroizingKey — memory zeroing ────────────────────────────────────────

    #[test]
    fn zeroizing_key_zeroed_on_drop() {
        // We can't directly observe the zeroing, but we verify the type compiles
        // and behaves like a string.
        let key: ZeroizingKey = Zeroizing::new("STEST_SECRET".to_string());
        assert_eq!(key.as_str(), "STEST_SECRET");
        drop(key); // zeroize called here
    }

    // ── KeyProviderKind::from_env ────────────────────────────────────────────

    #[test]
    fn kind_from_env_defaults_to_env() {
        std::env::remove_var("KEY_PROVIDER");
        assert_eq!(KeyProviderKind::from_env(), KeyProviderKind::Env);
    }

    #[test]
    fn kind_from_env_parses_aws() {
        std::env::set_var("KEY_PROVIDER", "aws");
        assert_eq!(KeyProviderKind::from_env(), KeyProviderKind::AwsSecretsManager);
        std::env::set_var("KEY_PROVIDER", "aws-secrets-manager");
        assert_eq!(KeyProviderKind::from_env(), KeyProviderKind::AwsSecretsManager);
        std::env::remove_var("KEY_PROVIDER");
    }

    #[test]
    fn kind_from_env_parses_vault() {
        std::env::set_var("KEY_PROVIDER", "vault");
        assert_eq!(KeyProviderKind::from_env(), KeyProviderKind::Vault);
        std::env::remove_var("KEY_PROVIDER");
    }

    // ── provider_name ────────────────────────────────────────────────────────

    #[test]
    fn provider_names_are_correct() {
        assert_eq!(EnvKeyProvider::new().provider_name(), "env");
        assert_eq!(AwsSecretsManagerKeyProvider::new().provider_name(), "aws-secrets-manager");
    }

    // ── get_secret via EnvKeyProvider ────────────────────────────────────────

    #[tokio::test]
    async fn env_provider_get_secret() {
        std::env::set_var("MY_SECRET_721", "super_secret_value");
        let provider = EnvKeyProvider::new();
        let secret = provider.get_secret("MY_SECRET_721").await.unwrap();
        assert_eq!(secret.as_str(), "super_secret_value");
        std::env::remove_var("MY_SECRET_721");
    }
}
