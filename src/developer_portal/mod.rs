//! Developer Portal Module
//!
//! Provides sandbox environment for external developers to test integrations
//! with Aframp platform. Features include:
//! - Sandbox isolation using Stellar Testnet
//! - Mock payment providers
//! - API key scoping to prevent production access
//! - Sandbox reset endpoint for test data cleanup

pub mod config;
pub mod routes;
pub mod sandbox;
pub mod models;
pub mod services;

use std::sync::Arc;

/// Developer portal configuration
#[derive(Clone, Debug)]
pub struct DeveloperPortalConfig {
    /// Whether sandbox mode is enabled
    pub sandbox_enabled: bool,
    /// Stellar Testnet Horizon URL
    pub stellar_testnet_url: String,
    /// Sandbox database connection string (separate from production)
    pub sandbox_database_url: Option<String>,
    /// Default rate limits for sandbox environment
    pub sandbox_rate_limit_per_minute: u32,
    /// Whether to allow sandbox reset operations
    pub allow_sandbox_reset: bool,
    /// Maximum sandbox data lifetime in hours
    pub max_sandbox_lifetime_hours: u32,
}

impl Default for DeveloperPortalConfig {
    fn default() -> Self {
        Self {
            sandbox_enabled: true,
            stellar_testnet_url: "https://horizon-testnet.stellar.org".to_string(),
            sandbox_database_url: None,
            sandbox_rate_limit_per_minute: 100,
            allow_sandbox_reset: true,
            max_sandbox_lifetime_hours: 24,
        }
    }
}

impl DeveloperPortalConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            sandbox_enabled: std::env::var("SANDBOX_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            stellar_testnet_url: std::env::var("STELLAR_TESTNET_URL")
                .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".to_string()),
            sandbox_database_url: std::env::var("SANDBOX_DATABASE_URL").ok(),
            sandbox_rate_limit_per_minute: std::env::var("SANDBOX_RATE_LIMIT_PER_MINUTE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            allow_sandbox_reset: std::env::var("ALLOW_SANDBOX_RESET")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            max_sandbox_lifetime_hours: std::env::var("MAX_SANDBOX_LIFETIME_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
        }
    }
}