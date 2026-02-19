use crate::database::error::{DatabaseError, DatabaseErrorKind};
use crate::database::repository::{Repository, TransactionalRepository};
use async_trait::async_trait;
use sqlx::{FromRow, PgPool};

/// Payment Provider Configuration entity
#[derive(Debug, Clone, FromRow)]
pub struct ProviderConfig {
    pub provider: String,
    pub is_enabled: bool,
    pub settings: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Repository for managing payment provider configurations
pub struct ProviderConfigRepository {
    pool: PgPool,
}

impl ProviderConfigRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find provider configuration by provider name
    pub async fn find_by_provider(
        &self,
        provider: &str,
    ) -> Result<Option<ProviderConfig>, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "SELECT provider, is_enabled, settings, created_at, updated_at 
             FROM payment_provider_configs 
             WHERE provider = $1",
        )
        .bind(provider)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Get all enabled providers
    pub async fn find_enabled(&self) -> Result<Vec<ProviderConfig>, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "SELECT provider, is_enabled, settings, created_at, updated_at 
             FROM payment_provider_configs 
             WHERE is_enabled = true 
             ORDER BY provider",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Enable a provider
    pub async fn enable_provider(&self, provider: &str) -> Result<ProviderConfig, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "UPDATE payment_provider_configs 
             SET is_enabled = true 
             WHERE provider = $1 
             RETURNING provider, is_enabled, settings, created_at, updated_at",
        )
        .bind(provider)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Disable a provider
    pub async fn disable_provider(&self, provider: &str) -> Result<ProviderConfig, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "UPDATE payment_provider_configs 
             SET is_enabled = false 
             WHERE provider = $1 
             RETURNING provider, is_enabled, settings, created_at, updated_at",
        )
        .bind(provider)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Update provider settings
    pub async fn update_settings(
        &self,
        provider: &str,
        settings: serde_json::Value,
    ) -> Result<ProviderConfig, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "UPDATE payment_provider_configs 
             SET settings = $2 
             WHERE provider = $1 
             RETURNING provider, is_enabled, settings, created_at, updated_at",
        )
        .bind(provider)
        .bind(settings)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Create or update provider configuration
    pub async fn upsert(
        &self,
        provider: &str,
        is_enabled: bool,
        settings: serde_json::Value,
    ) -> Result<ProviderConfig, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "INSERT INTO payment_provider_configs (provider, is_enabled, settings) 
             VALUES ($1, $2, $3) 
             ON CONFLICT (provider) 
             DO UPDATE SET is_enabled = $2, settings = $3 
             RETURNING provider, is_enabled, settings, created_at, updated_at",
        )
        .bind(provider)
        .bind(is_enabled)
        .bind(settings)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }
}

#[async_trait]
impl Repository for ProviderConfigRepository {
    type Entity = ProviderConfig;

    async fn find_by_id(&self, id: &str) -> Result<Option<Self::Entity>, DatabaseError> {
        self.find_by_provider(id).await
    }

    async fn find_all(&self) -> Result<Vec<Self::Entity>, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "SELECT provider, is_enabled, settings, created_at, updated_at 
             FROM payment_provider_configs 
             ORDER BY provider",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn insert(&self, entity: &Self::Entity) -> Result<Self::Entity, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "INSERT INTO payment_provider_configs (provider, is_enabled, settings) 
             VALUES ($1, $2, $3) 
             RETURNING provider, is_enabled, settings, created_at, updated_at",
        )
        .bind(&entity.provider)
        .bind(entity.is_enabled)
        .bind(&entity.settings)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn update(&self, id: &str, entity: &Self::Entity) -> Result<Self::Entity, DatabaseError> {
        sqlx::query_as::<_, ProviderConfig>(
            "UPDATE payment_provider_configs 
             SET is_enabled = $2, settings = $3 
             WHERE provider = $1 
             RETURNING provider, is_enabled, settings, created_at, updated_at",
        )
        .bind(id)
        .bind(entity.is_enabled)
        .bind(&entity.settings)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn delete(&self, id: &str) -> Result<bool, DatabaseError> {
        let result = sqlx::query("DELETE FROM payment_provider_configs WHERE provider = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(DatabaseError::from_sqlx)?;

        Ok(result.rows_affected() > 0)
    }
}

impl TransactionalRepository for ProviderConfigRepository {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_creation() {
        let config = ProviderConfig {
            provider: "flutterwave".to_string(),
            is_enabled: true,
            settings: serde_json::json!({"api_key": "test"}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(config.provider, "flutterwave");
        assert!(config.is_enabled);
    }
}
