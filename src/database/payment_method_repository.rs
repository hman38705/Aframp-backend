use crate::database::error::{DatabaseError, DatabaseErrorKind};
use crate::database::repository::{Repository, TransactionalRepository};
use async_trait::async_trait;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Payment Method entity
#[derive(Debug, Clone, FromRow)]
pub struct PaymentMethod {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub method_type: String,
    pub phone_number: Option<String>,
    pub encrypted_data: Option<String>,
    pub is_active: bool,
    pub is_deleted: bool,
    pub region: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Repository for managing user payment methods
pub struct PaymentMethodRepository {
    pool: PgPool,
}

impl PaymentMethodRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find payment methods by user ID
    pub async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<PaymentMethod>, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "SELECT id, user_id, provider, method_type, phone_number, encrypted_data, 
                    is_active, is_deleted, region, created_at, updated_at 
             FROM payment_methods 
             WHERE user_id = $1 AND is_deleted = false 
             ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Find active payment methods by user ID
    pub async fn find_active_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<PaymentMethod>, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "SELECT id, user_id, provider, method_type, phone_number, encrypted_data, 
                    is_active, is_deleted, region, created_at, updated_at 
             FROM payment_methods 
             WHERE user_id = $1 AND is_active = true AND is_deleted = false 
             ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Create a new payment method
    pub async fn create_payment_method(
        &self,
        user_id: Uuid,
        provider: &str,
        method_type: &str,
        phone_number: Option<&str>,
        encrypted_data: Option<&str>,
        region: Option<&str>,
    ) -> Result<PaymentMethod, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "INSERT INTO payment_methods 
             (user_id, provider, method_type, phone_number, encrypted_data, region) 
             VALUES ($1, $2, $3, $4, $5, $6) 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(user_id)
        .bind(provider)
        .bind(method_type)
        .bind(phone_number)
        .bind(encrypted_data)
        .bind(region)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Soft delete a payment method
    pub async fn soft_delete(&self, id: Uuid) -> Result<PaymentMethod, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "UPDATE payment_methods 
             SET is_deleted = true, is_active = false 
             WHERE id = $1 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Activate a payment method
    pub async fn activate(&self, id: Uuid) -> Result<PaymentMethod, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "UPDATE payment_methods 
             SET is_active = true 
             WHERE id = $1 AND is_deleted = false 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Deactivate a payment method
    pub async fn deactivate(&self, id: Uuid) -> Result<PaymentMethod, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "UPDATE payment_methods 
             SET is_active = false 
             WHERE id = $1 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Update encrypted data for a payment method
    pub async fn update_encrypted_data(
        &self,
        id: Uuid,
        encrypted_data: &str,
    ) -> Result<PaymentMethod, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "UPDATE payment_methods 
             SET encrypted_data = $2 
             WHERE id = $1 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(id)
        .bind(encrypted_data)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Find payment method by ID and user ID (for authorization)
    pub async fn find_by_id_and_user(
        &self,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<PaymentMethod>, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "SELECT id, user_id, provider, method_type, phone_number, encrypted_data, 
                    is_active, is_deleted, region, created_at, updated_at 
             FROM payment_methods 
             WHERE id = $1 AND user_id = $2 AND is_deleted = false",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }
}

#[async_trait]
impl Repository for PaymentMethodRepository {
    type Entity = PaymentMethod;

    async fn find_by_id(&self, id: &str) -> Result<Option<Self::Entity>, DatabaseError> {
        let uuid = Uuid::parse_str(id).map_err(|e| {
            DatabaseError::new(DatabaseErrorKind::Unknown {
                message: format!("Invalid UUID: {}", e),
            })
        })?;

        sqlx::query_as::<_, PaymentMethod>(
            "SELECT id, user_id, provider, method_type, phone_number, encrypted_data, 
                    is_active, is_deleted, region, created_at, updated_at 
             FROM payment_methods 
             WHERE id = $1",
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn find_all(&self) -> Result<Vec<Self::Entity>, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "SELECT id, user_id, provider, method_type, phone_number, encrypted_data, 
                    is_active, is_deleted, region, created_at, updated_at 
             FROM payment_methods 
             WHERE is_deleted = false 
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn insert(&self, entity: &Self::Entity) -> Result<Self::Entity, DatabaseError> {
        sqlx::query_as::<_, PaymentMethod>(
            "INSERT INTO payment_methods 
             (user_id, provider, method_type, phone_number, encrypted_data, is_active, region) 
             VALUES ($1, $2, $3, $4, $5, $6, $7) 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(entity.user_id)
        .bind(&entity.provider)
        .bind(&entity.method_type)
        .bind(&entity.phone_number)
        .bind(&entity.encrypted_data)
        .bind(entity.is_active)
        .bind(&entity.region)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn update(&self, id: &str, entity: &Self::Entity) -> Result<Self::Entity, DatabaseError> {
        let uuid = Uuid::parse_str(id).map_err(|e| {
            DatabaseError::new(DatabaseErrorKind::Unknown {
                message: format!("Invalid UUID: {}", e),
            })
        })?;

        sqlx::query_as::<_, PaymentMethod>(
            "UPDATE payment_methods 
             SET provider = $2, method_type = $3, phone_number = $4, encrypted_data = $5, 
                 is_active = $6, region = $7 
             WHERE id = $1 
             RETURNING id, user_id, provider, method_type, phone_number, encrypted_data, 
                       is_active, is_deleted, region, created_at, updated_at",
        )
        .bind(uuid)
        .bind(&entity.provider)
        .bind(&entity.method_type)
        .bind(&entity.phone_number)
        .bind(&entity.encrypted_data)
        .bind(entity.is_active)
        .bind(&entity.region)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    async fn delete(&self, id: &str) -> Result<bool, DatabaseError> {
        let uuid = Uuid::parse_str(id).map_err(|e| {
            DatabaseError::new(DatabaseErrorKind::Unknown {
                message: format!("Invalid UUID: {}", e),
            })
        })?;

        // Soft delete
        let result = sqlx::query(
            "UPDATE payment_methods SET is_deleted = true, is_active = false WHERE id = $1",
        )
        .bind(uuid)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(result.rows_affected() > 0)
    }
}

impl TransactionalRepository for PaymentMethodRepository {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_method_creation() {
        let method = PaymentMethod {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            provider: "paystack".to_string(),
            method_type: "card".to_string(),
            phone_number: None,
            encrypted_data: Some("encrypted_token".to_string()),
            is_active: true,
            is_deleted: false,
            region: Some("NG".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(method.provider, "paystack");
        assert_eq!(method.method_type, "card");
        assert!(method.is_active);
        assert!(!method.is_deleted);
    }
}
