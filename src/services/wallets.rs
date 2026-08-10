use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{NewWallet, Wallet};

pub async fn create_wallet(
    db: &PgPool,
    merchant_id: Uuid,
    network: &str,
) -> Result<Wallet, sqlx::Error> {
    let address = format!("A{:x}{:x}", merchant_id.simple(), network.len());
    let wallet = NewWallet {
        merchant_id,
        address,
        network: network.to_string(),
    };
    sqlx::query_as::<_, Wallet>(
        "INSERT INTO wallets (merchant_id, address, network)
         VALUES ($1, $2, $3)
         RETURNING id, merchant_id, address, network, created_at",
    )
    .bind(wallet.merchant_id)
    .bind(&wallet.address)
    .bind(&wallet.network)
    .fetch_one(db)
    .await
}

pub async fn wallet_by_merchant(
    db: &PgPool,
    merchant_id: Uuid,
) -> Result<Option<Wallet>, sqlx::Error> {
    sqlx::query_as::<_, Wallet>(
        "SELECT id, merchant_id, address, network, created_at
           FROM wallets
          WHERE merchant_id = $1
          ORDER BY created_at DESC
          LIMIT 1",
    )
    .bind(merchant_id)
    .fetch_optional(db)
    .await
}

pub async fn wallet_by_address(db: &PgPool, address: &str) -> Result<Option<Wallet>, sqlx::Error> {
    sqlx::query_as::<_, Wallet>(
        "SELECT id, merchant_id, address, network, created_at FROM wallets WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(db)
    .await
}
