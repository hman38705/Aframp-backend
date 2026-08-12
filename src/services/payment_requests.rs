use chrono::{Duration, Utc};
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::PaymentRequest;

const DEFAULT_EXPIRY_SECS: i64 = 15 * 60;
const MIN_EXPIRY_SECS: i64 = 60;
const MAX_EXPIRY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, thiserror::Error)]
pub enum PaymentRequestError {
    #[error("amount_stroops must be positive")]
    InvalidAmount,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

fn generate_memo() -> String {
    let mut bytes = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub async fn create_payment_request(
    db: &PgPool,
    merchant_id: Uuid,
    wallet_id: Uuid,
    amount_stroops: i64,
    asset: String,
    expires_in_secs: Option<i64>,
) -> Result<PaymentRequest, PaymentRequestError> {
    if amount_stroops <= 0 {
        return Err(PaymentRequestError::InvalidAmount);
    }
    let ttl = expires_in_secs
        .unwrap_or(DEFAULT_EXPIRY_SECS)
        .clamp(MIN_EXPIRY_SECS, MAX_EXPIRY_SECS);
    let expires_at = Utc::now() + Duration::seconds(ttl);
    let memo = generate_memo();

    sqlx::query_as::<_, PaymentRequest>(
        "INSERT INTO payment_requests (merchant_id, wallet_id, amount_stroops, asset, memo, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, merchant_id, wallet_id, amount_stroops, asset, memo, status, payment_id,
                   expires_at, created_at, updated_at",
    )
    .bind(merchant_id)
    .bind(wallet_id)
    .bind(amount_stroops)
    .bind(&asset)
    .bind(&memo)
    .bind(expires_at)
    .fetch_one(db)
    .await
    .map_err(PaymentRequestError::from)
}

pub async fn payment_request_by_id(db: &PgPool, id: Uuid) -> Result<Option<PaymentRequest>, sqlx::Error> {
    sqlx::query_as::<_, PaymentRequest>(
        "SELECT id, merchant_id, wallet_id, amount_stroops, asset, memo, status, payment_id,
                expires_at, created_at, updated_at
           FROM payment_requests WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

/// Looks up the pending request a detected deposit's memo correlates to, if any.
pub async fn find_pending_by_wallet_and_memo(
    db: &PgPool,
    wallet_id: Uuid,
    memo: &str,
) -> Result<Option<PaymentRequest>, sqlx::Error> {
    sqlx::query_as::<_, PaymentRequest>(
        "SELECT id, merchant_id, wallet_id, amount_stroops, asset, memo, status, payment_id,
                expires_at, created_at, updated_at
           FROM payment_requests
          WHERE wallet_id = $1 AND memo = $2 AND status = 'pending'",
    )
    .bind(wallet_id)
    .bind(memo)
    .fetch_optional(db)
    .await
}

pub async fn mark_paid(db: &PgPool, id: Uuid, payment_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE payment_requests SET status = 'paid', payment_id = $2, updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(payment_id)
    .execute(db)
    .await
    .map(|_| ())
}
