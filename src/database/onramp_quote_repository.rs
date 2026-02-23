use crate::database::error::DatabaseError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Onramp quote entity
#[derive(Debug, Clone, FromRow)]
pub struct OnrampQuote {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub amount_ngn: sqlx::types::BigDecimal,
    pub exchange_rate: sqlx::types::BigDecimal,
    pub gross_cngn: sqlx::types::BigDecimal,
    pub fee_cngn: sqlx::types::BigDecimal,
    pub net_cngn: sqlx::types::BigDecimal,
    pub status: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Repository for onramp quotes
pub struct OnrampQuoteRepository {
    pool: PgPool,
}

impl OnrampQuoteRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new onramp quote
    pub async fn create(
        &self,
        amount_ngn: &sqlx::types::BigDecimal,
        exchange_rate: &sqlx::types::BigDecimal,
        gross_cngn: &sqlx::types::BigDecimal,
        fee_cngn: &sqlx::types::BigDecimal,
        net_cngn: &sqlx::types::BigDecimal,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<OnrampQuote, DatabaseError> {
        sqlx::query_as::<_, OnrampQuote>(
            r#"
            INSERT INTO onramp_quotes
                (amount_ngn, exchange_rate, gross_cngn, fee_cngn, net_cngn, status, expires_at)
            VALUES ($1, $2, $3, $4, $5, 'pending', $6)
            RETURNING id, quote_id, amount_ngn, exchange_rate, gross_cngn, fee_cngn, net_cngn, status, expires_at, created_at, updated_at
            "#,
        )
        .bind(amount_ngn)
        .bind(exchange_rate)
        .bind(gross_cngn)
        .bind(fee_cngn)
        .bind(net_cngn)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Find quote by quote_id
    pub async fn find_by_quote_id(
        &self,
        quote_id: Uuid,
    ) -> Result<Option<OnrampQuote>, DatabaseError> {
        sqlx::query_as::<_, OnrampQuote>(
            r#"
            SELECT id, quote_id, amount_ngn, exchange_rate, gross_cngn, fee_cngn, net_cngn, status, expires_at, created_at, updated_at
            FROM onramp_quotes
            WHERE quote_id = $1
            "#,
        )
        .bind(quote_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Mark quote as consumed
    pub async fn mark_consumed(&self, quote_id: Uuid) -> Result<bool, DatabaseError> {
        let result = sqlx::query(
            "UPDATE onramp_quotes SET status = 'consumed', updated_at = NOW() WHERE quote_id = $1 AND status = 'pending'",
        )
        .bind(quote_id)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(result.rows_affected() > 0)
    }

    /// Mark expired quotes
    pub async fn mark_expired(&self) -> Result<u64, DatabaseError> {
        let result = sqlx::query(
            "UPDATE onramp_quotes SET status = 'expired', updated_at = NOW() WHERE status = 'pending' AND expires_at < NOW()",
        )
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(result.rows_affected())
    }
}
