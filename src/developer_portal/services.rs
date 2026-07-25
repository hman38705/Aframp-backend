//! Developer portal services

use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api_keys::generator::{generate_api_key, KeyEnvironment};
use crate::developer_portal::models::{ApiKeyScope, DeveloperAccount, DeveloperApplication};
use crate::developer_portal::sandbox::{SandboxEnvironment, SandboxIsolationService};

/// Developer portal service
#[derive(Clone)]
pub struct DeveloperPortalService {
    sandbox_service: Arc<SandboxIsolationService>,
    db: Arc<sqlx::PgPool>,
}

impl DeveloperPortalService {
    /// Create new developer portal service
    pub fn new(db: Arc<sqlx::PgPool>, sandbox_service: Arc<SandboxIsolationService>) -> Self {
        Self {
            sandbox_service,
            db,
        }
    }

    /// Create sandbox API key for developer
    pub async fn create_sandbox_api_key(
        &self,
        developer_account: &DeveloperAccount,
        application: &DeveloperApplication,
        scope: ApiKeyScope,
    ) -> Result<CreatedApiKey, anyhow::Error> {
        // Validate developer can access sandbox
        if !developer_account.can_access_sandbox() {
            return Err(anyhow::anyhow!(
                "Developer account cannot access sandbox environment"
            ));
        }

        // Validate scope is for sandbox
        if !scope.is_sandbox() {
            return Err(anyhow::anyhow!(
                "API key scope must be for sandbox environment"
            ));
        }

        // Generate API key for sandbox environment
        let (key_id, plaintext_key, key_prefix) = generate_api_key(
            &developer_account.id.to_string(),
            KeyEnvironment::Testnet,
            Some(scope.expires_at),
            Some(application.name.clone()),
        );

        // Store API key in database
        let api_key_record = sqlx::query!(
            r#"
            INSERT INTO api_keys (
                key_id, consumer_id, hashed_key, key_prefix, environment,
                status, description, expires_at, scope
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, created_at
            "#,
            key_id,
            developer_account.id,
            crate::api_keys::generator::hash_api_key(&plaintext_key),
            key_prefix,
            "testnet",
            "active",
            format!("Sandbox key for application: {}", application.name),
            scope.expires_at,
            serde_json::to_value(&scope).ok(),
        )
        .fetch_one(self.db.as_ref())
        .await?;

        info!(
            developer_id = %developer_account.id,
            application_id = %application.id,
            key_id = %key_id,
            "Sandbox API key created"
        );

        Ok(CreatedApiKey {
            key_id,
            plaintext_key,
            key_prefix,
            scope,
            created_at: api_key_record.created_at,
        })
    }

    /// Validate sandbox API key
    pub async fn validate_sandbox_key(
        &self,
        key_id: &str,
        requested_environment: SandboxEnvironment,
    ) -> Result<ValidationResult, anyhow::Error> {
        // Look up API key
        let api_key = sqlx::query!(
            r#"
            SELECT 
                key_id, consumer_id, environment, status, expires_at, scope
            FROM api_keys
            WHERE key_id = $1 AND environment = 'testnet'
            "#,
            key_id
        )
        .fetch_optional(self.db.as_ref())
        .await?;

        let api_key = match api_key {
            Some(key) => key,
            None => {
                return Ok(ValidationResult::InvalidKey);
            }
        };

        // Check key status
        if api_key.status != "active" {
            return Ok(ValidationResult::InvalidKey);
        }

        // Check expiration
        if let Some(expires_at) = api_key.expires_at {
            if expires_at < chrono::Utc::now() {
                return Ok(ValidationResult::ExpiredKey);
            }
        }

        // Parse scope
        let scope: Option<ApiKeyScope> = api_key
            .scope
            .and_then(|s| serde_json::from_value(s).ok());

        // Validate scope if present
        if let Some(scope) = scope {
            if !scope.is_sandbox() {
                return Ok(ValidationResult::InvalidScope);
            }

            // Additional scope validation could go here
        }

        Ok(ValidationResult::Valid {
            consumer_id: api_key.consumer_id,
            environment: SandboxEnvironment::Testnet, // Sandbox keys always testnet
        })
    }

    /// Get developer account by ID
    pub async fn get_developer_account(
        &self,
        developer_id: Uuid,
    ) -> Result<Option<DeveloperAccount>, anyhow::Error> {
        let account = sqlx::query_as!(
            DeveloperAccount,
            r#"
            SELECT 
                id, email, full_name, organisation, country, use_case_description,
                status_code, access_tier_code, email_verified, email_verification_token,
                email_verification_expires_at, identity_verification_status,
                identity_verification_data, identity_verified_at, suspended_at,
                suspension_reason, created_at, updated_at
            FROM developer_accounts
            WHERE id = $1
            "#,
            developer_id
        )
        .fetch_optional(self.db.as_ref())
        .await?;

        Ok(account)
    }

    /// Get developer applications
    pub async fn get_developer_applications(
        &self,
        developer_id: Uuid,
    ) -> Result<Vec<DeveloperApplication>, anyhow::Error> {
        let applications = sqlx::query_as!(
            DeveloperApplication,
            r#"
            SELECT 
                id, developer_account_id, name, description, intended_use_case,
                status, sandbox_wallet_address, sandbox_wallet_secret,
                mainnet_wallet_address, mainnet_wallet_secret, webhook_url,
                webhook_secret, created_at, updated_at
            FROM developer_applications
            WHERE developer_account_id = $1 AND status != 'deleted'
            ORDER BY created_at DESC
            "#,
            developer_id
        )
        .fetch_all(self.db.as_ref())
        .await?;

        Ok(applications)
    }

    /// Create sandbox wallet for application
    pub async fn create_sandbox_wallet(
        &self,
        application: &DeveloperApplication,
    ) -> Result<SandboxWallet, anyhow::Error> {
        // Generate Stellar testnet wallet
        // In a real implementation, this would generate actual Stellar keypairs
        let wallet_address = format!("GCTEST{}", Uuid::new_v4().to_string().replace("-", ""));
        let wallet_secret = format!("SCTEST{}", Uuid::new_v4().to_string().replace("-", ""));

        // Update application with wallet info
        sqlx::query!(
            r#"
            UPDATE developer_applications
            SET 
                sandbox_wallet_address = $1,
                sandbox_wallet_secret = $2,
                updated_at = now()
            WHERE id = $3
            "#,
            wallet_address,
            wallet_secret,
            application.id
        )
        .execute(self.db.as_ref())
        .await?;

        info!(
            application_id = %application.id,
            wallet_address = %wallet_address,
            "Sandbox wallet created"
        );

        Ok(SandboxWallet {
            address: wallet_address,
            secret: wallet_secret,
            network: "testnet".to_string(),
            created_at: chrono::Utc::now(),
        })
    }

    /// Clean up old sandbox data
    pub async fn cleanup_old_sandbox_data(&self, max_age_hours: i32) -> Result<CleanupStats, anyhow::Error> {
        let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);

        // Clean up old sandbox transactions
        let transactions_deleted = sqlx::query!(
            r#"
            DELETE FROM transactions 
            WHERE environment = 'testnet' 
            AND created_at < $1
            "#,
            cutoff_time
        )
        .execute(self.db.as_ref())
        .await?
        .rows_affected();

        // Clean up old sandbox payments
        let payments_deleted = sqlx::query!(
            r#"
            DELETE FROM payments 
            WHERE environment = 'testnet' 
            AND created_at < $1
            "#,
            cutoff_time
        )
        .execute(self.db.as_ref())
        .await?
        .rows_affected();

        // Clean up expired sandbox API keys
        let keys_deleted = sqlx::query!(
            r#"
            DELETE FROM api_keys 
            WHERE environment = 'testnet' 
            AND expires_at < $1
            "#,
            chrono::Utc::now()
        )
        .execute(self.db.as_ref())
        .await?
        .rows_affected();

        info!(
            transactions_deleted = %transactions_deleted,
            payments_deleted = %payments_deleted,
            keys_deleted = %keys_deleted,
            max_age_hours = %max_age_hours,
            "Sandbox data cleanup completed"
        );

        Ok(CleanupStats {
            transactions_deleted: transactions_deleted as u64,
            payments_deleted: payments_deleted as u64,
            keys_deleted: keys_deleted as u64,
            cutoff_time,
        })
    }
}

/// Created API key response
#[derive(Debug, Clone)]
pub struct CreatedApiKey {
    pub key_id: String,
    pub plaintext_key: String,
    pub key_prefix: String,
    pub scope: ApiKeyScope,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// API key validation result
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid {
        consumer_id: Uuid,
        environment: SandboxEnvironment,
    },
    InvalidKey,
    ExpiredKey,
    InvalidScope,
    RateLimitExceeded,
}

/// Sandbox wallet
#[derive(Debug, Clone)]
pub struct SandboxWallet {
    pub address: String,
    pub secret: String,
    pub network: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Cleanup statistics
#[derive(Debug, Clone)]
pub struct CleanupStats {
    pub transactions_deleted: u64,
    pub payments_deleted: u64,
    pub keys_deleted: u64,
    pub cutoff_time: chrono::DateTime<chrono::Utc>,
}