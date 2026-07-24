//! Developer portal data models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use uuid::Uuid;

/// Developer account status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum DeveloperAccountStatus {
    Unverified,
    Verified,
    IdentityPending,
    IdentityVerified,
    IdentityRejected,
    Suspended,
    Active,
}

impl std::fmt::Display for DeveloperAccountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeveloperAccountStatus::Unverified => write!(f, "unverified"),
            DeveloperAccountStatus::Verified => write!(f, "verified"),
            DeveloperAccountStatus::IdentityPending => write!(f, "identity_pending"),
            DeveloperAccountStatus::IdentityVerified => write!(f, "identity_verified"),
            DeveloperAccountStatus::IdentityRejected => write!(f, "identity_rejected"),
            DeveloperAccountStatus::Suspended => write!(f, "suspended"),
            DeveloperAccountStatus::Active => write!(f, "active"),
        }
    }
}

/// Access tier for developers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum AccessTier {
    Sandbox,
    Standard,
    Partner,
}

impl std::fmt::Display for AccessTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessTier::Sandbox => write!(f, "sandbox"),
            AccessTier::Standard => write!(f, "standard"),
            AccessTier::Partner => write!(f, "partner"),
        }
    }
}

impl AccessTier {
    /// Get maximum number of applications allowed for this tier
    pub fn max_applications(&self) -> i32 {
        match self {
            AccessTier::Sandbox => 3,
            AccessTier::Standard => 10,
            AccessTier::Partner => 50,
        }
    }

    /// Get rate limit per minute for this tier
    pub fn rate_limit_per_minute(&self) -> i32 {
        match self {
            AccessTier::Sandbox => 50,
            AccessTier::Standard => 1000,
            AccessTier::Partner => 10000,
        }
    }

    /// Check if identity verification is required
    pub fn requires_identity_verification(&self) -> bool {
        match self {
            AccessTier::Sandbox => false,
            AccessTier::Standard => true,
            AccessTier::Partner => true,
        }
    }

    /// Check if business agreement is required
    pub fn requires_business_agreement(&self) -> bool {
        match self {
            AccessTier::Sandbox => false,
            AccessTier::Standard => false,
            AccessTier::Partner => true,
        }
    }
}

/// Developer account
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeveloperAccount {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub organisation: Option<String>,
    pub country: String,
    pub use_case_description: String,
    pub status_code: String,
    pub access_tier_code: String,
    pub email_verified: bool,
    pub email_verification_token: Option<String>,
    pub email_verification_expires_at: Option<DateTime<Utc>>,
    pub identity_verification_status: Option<String>,
    pub identity_verification_data: Option<serde_json::Value>,
    pub identity_verified_at: Option<DateTime<Utc>>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspension_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DeveloperAccount {
    /// Get account status enum
    pub fn status(&self) -> DeveloperAccountStatus {
        match self.status_code.as_str() {
            "unverified" => DeveloperAccountStatus::Unverified,
            "verified" => DeveloperAccountStatus::Verified,
            "identity_pending" => DeveloperAccountStatus::IdentityPending,
            "identity_verified" => DeveloperAccountStatus::IdentityVerified,
            "identity_rejected" => DeveloperAccountStatus::IdentityRejected,
            "suspended" => DeveloperAccountStatus::Suspended,
            "active" => DeveloperAccountStatus::Active,
            _ => DeveloperAccountStatus::Unverified,
        }
    }

    /// Get access tier enum
    pub fn access_tier(&self) -> AccessTier {
        match self.access_tier_code.as_str() {
            "sandbox" => AccessTier::Sandbox,
            "standard" => AccessTier::Standard,
            "partner" => AccessTier::Partner,
            _ => AccessTier::Sandbox,
        }
    }

    /// Check if account can access sandbox
    pub fn can_access_sandbox(&self) -> bool {
        self.email_verified && matches!(self.status(), DeveloperAccountStatus::Verified | DeveloperAccountStatus::Active)
    }

    /// Check if account can access production
    pub fn can_access_production(&self) -> bool {
        self.email_verified 
            && matches!(self.status(), DeveloperAccountStatus::Active)
            && self.access_tier() != AccessTier::Sandbox
    }
}

/// Developer application
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeveloperApplication {
    pub id: Uuid,
    pub developer_account_id: Uuid,
    pub name: String,
    pub description: String,
    pub intended_use_case: String,
    pub status: String,
    pub sandbox_wallet_address: Option<String>,
    pub sandbox_wallet_secret: Option<String>,
    pub mainnet_wallet_address: Option<String>,
    pub mainnet_wallet_secret: Option<String>,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DeveloperApplication {
    /// Check if application is active
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    /// Get sandbox wallet info if available
    pub fn sandbox_wallet_info(&self) -> Option<WalletInfo> {
        self.sandbox_wallet_address.as_ref().map(|address| WalletInfo {
            address: address.clone(),
            secret: self.sandbox_wallet_secret.clone(),
        })
    }

    /// Get mainnet wallet info if available
    pub fn mainnet_wallet_info(&self) -> Option<WalletInfo> {
        self.mainnet_wallet_address.as_ref().map(|address| WalletInfo {
            address: address.clone(),
            secret: self.mainnet_wallet_secret.clone(),
        })
    }
}

/// Wallet information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub address: String,
    pub secret: Option<String>,
}

/// API key scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyScope {
    pub environment: String,
    pub resources: Vec<String>,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl ApiKeyScope {
    /// Create sandbox scope
    pub fn sandbox() -> Self {
        Self {
            environment: "testnet".to_string(),
            resources: vec![
                "transactions".to_string(),
                "wallets".to_string(),
                "payments".to_string(),
                "balances".to_string(),
            ],
            permissions: vec!["read".to_string(), "write".to_string()],
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(24)),
        }
    }

    /// Create production scope
    pub fn production() -> Self {
        Self {
            environment: "mainnet".to_string(),
            resources: vec![
                "transactions".to_string(),
                "wallets".to_string(),
                "payments".to_string(),
                "balances".to_string(),
                "accounts".to_string(),
            ],
            permissions: vec!["read".to_string(), "write".to_string(), "admin".to_string()],
            expires_at: None, // Production keys don't auto-expire
        }
    }

    /// Check if scope includes resource
    pub fn has_resource(&self, resource: &str) -> bool {
        self.resources.iter().any(|r| r == resource)
    }

    /// Check if scope includes permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// Check if scope is for sandbox
    pub fn is_sandbox(&self) -> bool {
        self.environment == "testnet"
    }
}