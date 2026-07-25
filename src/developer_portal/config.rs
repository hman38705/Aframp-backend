//! Developer portal configuration

use serde::{Deserialize, Serialize};

/// Developer portal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperPortalConfig {
    /// Whether the developer portal is enabled
    pub enabled: bool,
    
    /// Sandbox-specific configuration
    pub sandbox: SandboxConfig,
    
    /// API key scoping configuration
    pub api_key_scoping: ApiKeyScopingConfig,
    
    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Whether sandbox mode is enabled
    pub enabled: bool,
    
    /// Stellar Testnet Horizon URL
    pub stellar_testnet_url: String,
    
    /// Sandbox database connection string (optional, uses main DB if None)
    pub database_url: Option<String>,
    
    /// Whether to allow sandbox reset operations
    pub allow_reset: bool,
    
    /// Maximum sandbox data lifetime in hours
    pub max_lifetime_hours: u32,
    
    /// Default starting balance for sandbox wallets
    pub default_starting_balance: f64,
    
    /// Mock payment provider configuration
    pub mock_payments: MockPaymentsConfig,
}

/// Mock payments configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockPaymentsConfig {
    /// Whether mock payments are enabled
    pub enabled: bool,
    
    /// Simulated payment processing delay in milliseconds
    pub processing_delay_ms: u64,
    
    /// Success rate for mock payments (0.0 to 1.0)
    pub success_rate: f64,
    
    /// Default currencies supported
    pub supported_currencies: Vec<String>,
}

/// API key scoping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyScopingConfig {
    /// Whether API key scoping is enforced
    pub enforced: bool,
    
    /// Default scope for sandbox keys
    pub default_sandbox_scope: ApiKeyScope,
    
    /// Default scope for production keys
    pub default_production_scope: ApiKeyScope,
    
    /// Whether to automatically expire sandbox keys
    pub auto_expire_sandbox_keys: bool,
    
    /// Sandbox key expiration in hours
    pub sandbox_key_expiration_hours: u32,
}

/// API key scope definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyScope {
    /// Environment: "testnet" or "mainnet"
    pub environment: String,
    
    /// Allowed resources
    pub resources: Vec<String>,
    
    /// Allowed permissions
    pub permissions: Vec<String>,
    
    /// Expiration timestamp
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Whether rate limiting is enabled
    pub enabled: bool,
    
    /// Rate limit per minute for sandbox environment
    pub sandbox_per_minute: u32,
    
    /// Rate limit per minute for production environment
    pub production_per_minute: u32,
    
    /// Burst allowance multiplier
    pub burst_multiplier: u32,
    
    /// Whether to rate limit by IP address
    pub limit_by_ip: bool,
    
    /// Whether to rate limit by API key
    pub limit_by_api_key: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether to require email verification
    pub require_email_verification: bool,
    
    /// Whether to require identity verification for production access
    pub require_identity_verification: bool,
    
    /// Whether to require business agreement for partner tier
    pub require_business_agreement: bool,
    
    /// Minimum password strength score (0-4)
    pub min_password_strength: u8,
    
    /// Whether to enforce HTTPS
    pub enforce_https: bool,
    
    /// Whether to log security events
    pub log_security_events: bool,
}

impl Default for DeveloperPortalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sandbox: SandboxConfig::default(),
            api_key_scoping: ApiKeyScopingConfig::default(),
            rate_limiting: RateLimitingConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stellar_testnet_url: "https://horizon-testnet.stellar.org".to_string(),
            database_url: None,
            allow_reset: true,
            max_lifetime_hours: 24,
            default_starting_balance: 1000.0,
            mock_payments: MockPaymentsConfig::default(),
        }
    }
}

impl Default for MockPaymentsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            processing_delay_ms: 100,
            success_rate: 0.95,
            supported_currencies: vec!["USD".to_string(), "EUR".to_string(), "GBP".to_string()],
        }
    }
}

impl Default for ApiKeyScopingConfig {
    fn default() -> Self {
        Self {
            enforced: true,
            default_sandbox_scope: ApiKeyScope {
                environment: "testnet".to_string(),
                resources: vec![
                    "transactions".to_string(),
                    "wallets".to_string(),
                    "payments".to_string(),
                    "balances".to_string(),
                ],
                permissions: vec!["read".to_string(), "write".to_string()],
                expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(24)),
            },
            default_production_scope: ApiKeyScope {
                environment: "mainnet".to_string(),
                resources: vec![
                    "transactions".to_string(),
                    "wallets".to_string(),
                    "payments".to_string(),
                    "balances".to_string(),
                    "accounts".to_string(),
                ],
                permissions: vec!["read".to_string(), "write".to_string(), "admin".to_string()],
                expires_at: None,
            },
            auto_expire_sandbox_keys: true,
            sandbox_key_expiration_hours: 24,
        }
    }
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sandbox_per_minute: 100,
            production_per_minute: 1000,
            burst_multiplier: 5,
            limit_by_ip: true,
            limit_by_api_key: true,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_email_verification: true,
            require_identity_verification: true,
            require_business_agreement: true,
            min_password_strength: 3,
            enforce_https: true,
            log_security_events: true,
        }
    }
}

impl DeveloperPortalConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("DEVELOPER_PORTAL_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            sandbox: SandboxConfig::from_env(),
            api_key_scoping: ApiKeyScopingConfig::from_env(),
            rate_limiting: RateLimitingConfig::from_env(),
            security: SecurityConfig::from_env(),
        }
    }
}

impl SandboxConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("SANDBOX_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            stellar_testnet_url: std::env::var("STELLAR_TESTNET_URL")
                .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".to_string()),
            
            database_url: std::env::var("SANDBOX_DATABASE_URL").ok(),
            
            allow_reset: std::env::var("ALLOW_SANDBOX_RESET")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            max_lifetime_hours: std::env::var("MAX_SANDBOX_LIFETIME_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            
            default_starting_balance: std::env::var("DEFAULT_SANDBOX_BALANCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000.0),
            
            mock_payments: MockPaymentsConfig::from_env(),
        }
    }
}

impl MockPaymentsConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("MOCK_PAYMENTS_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            processing_delay_ms: std::env::var("MOCK_PAYMENTS_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            
            success_rate: std::env::var("MOCK_PAYMENTS_SUCCESS_RATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.95),
            
            supported_currencies: std::env::var("MOCK_PAYMENTS_CURRENCIES")
                .ok()
                .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["USD".to_string(), "EUR".to_string(), "GBP".to_string()]),
        }
    }
}

impl ApiKeyScopingConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enforced: std::env::var("API_KEY_SCOPING_ENFORCED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            default_sandbox_scope: ApiKeyScope {
                environment: "testnet".to_string(),
                resources: std::env::var("SANDBOX_RESOURCES")
                    .ok()
                    .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_else(|| vec![
                        "transactions".to_string(),
                        "wallets".to_string(),
                        "payments".to_string(),
                        "balances".to_string(),
                    ]),
                permissions: std::env::var("SANDBOX_PERMISSIONS")
                    .ok()
                    .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_else(|| vec!["read".to_string(), "write".to_string()]),
                expires_at: std::env::var("SANDBOX_KEY_EXPIRATION_HOURS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .map(|hours: i64| chrono::Utc::now() + chrono::Duration::hours(hours)),
            },
            
            default_production_scope: ApiKeyScope {
                environment: "mainnet".to_string(),
                resources: std::env::var("PRODUCTION_RESOURCES")
                    .ok()
                    .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_else(|| vec![
                        "transactions".to_string(),
                        "wallets".to_string(),
                        "payments".to_string(),
                        "balances".to_string(),
                        "accounts".to_string(),
                    ]),
                permissions: std::env::var("PRODUCTION_PERMISSIONS")
                    .ok()
                    .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_else(|| vec!["read".to_string(), "write".to_string(), "admin".to_string()]),
                expires_at: None,
            },
            
            auto_expire_sandbox_keys: std::env::var("AUTO_EXPIRE_SANDBOX_KEYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            sandbox_key_expiration_hours: std::env::var("SANDBOX_KEY_EXPIRATION_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
        }
    }
}

impl RateLimitingConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("RATE_LIMITING_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            sandbox_per_minute: std::env::var("SANDBOX_RATE_LIMIT_PER_MINUTE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            
            production_per_minute: std::env::var("PRODUCTION_RATE_LIMIT_PER_MINUTE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            
            burst_multiplier: std::env::var("RATE_LIMIT_BURST_MULTIPLIER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            
            limit_by_ip: std::env::var("RATE_LIMIT_BY_IP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            limit_by_api_key: std::env::var("RATE_LIMIT_BY_API_KEY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }
    }
}

impl SecurityConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            require_email_verification: std::env::var("REQUIRE_EMAIL_VERIFICATION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            require_identity_verification: std::env::var("REQUIRE_IDENTITY_VERIFICATION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            require_business_agreement: std::env::var("REQUIRE_BUSINESS_AGREEMENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            min_password_strength: std::env::var("MIN_PASSWORD_STRENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            
            enforce_https: std::env::var("ENFORCE_HTTPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            log_security_events: std::env::var("LOG_SECURITY_EVENTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }
    }
}