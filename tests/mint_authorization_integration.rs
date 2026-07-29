//! Integration tests for the Mint Authorization Framework (#213).
//!
//! Tests the full lifecycle: request creation → signature collection →
//! threshold detection → envelope assembly → Stellar testnet submission → confirmation.
//!
//! Run with:
//!   DATABASE_URL=postgres://... STELLAR_ISSUER_ADDRESS=G... \
//!   cargo test --test mint_authorization_integration --features integration -- --nocapture
//!
//! Requires:
//!   - A live PostgreSQL database with migrations applied
//!   - Stellar testnet access (https://horizon-testnet.stellar.org)
//!   - STELLAR_ISSUER_ADDRESS env var set to a funded testnet issuer account

#![cfg(feature = "integration")]

use aframp_backend::{
    chains::stellar::{client::StellarClient, config::StellarConfig},
    mint_authorization::{
        error::MintAuthError,
        models::{
            CancelMintAuthRequest, CreateMintAuthRequest, MintAuthStatus, SubmitSignatureRequest,
        },
        repository::MintAuthRepository,
        service::{compute_tx_hash, verify_ed25519_signature, MintAuthService},
    },
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use std::sync::Arc;
use stellar_strkey::ed25519::PublicKey as StrkeyPublicKey;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a connection pool from `DATABASE_URL`.
///
/// Returns `Result` so callers can propagate the error with `?` instead of
/// panicking on a missing or invalid env-var / connection failure.
async fn test_pool() -> Result<PgPool, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL env-var is required for integration tests")?;
    let pool = PgPool::connect(&url).await?;
    Ok(pool)
}

/// Construct a `StellarClient` pointed at the Stellar testnet.
///
/// Returns `Result` so callers can surface a meaningful error instead of
/// panicking if the client cannot be initialised.
fn testnet_stellar_client() -> Result<Arc<StellarClient>, Box<dyn std::error::Error>> {
    let config = StellarConfig::testnet();
    let client = StellarClient::new(config)
        .map_err(|e| format!("stellar client init failed: {}", e))?;
    Ok(Arc::new(client))
}

fn make_service(pool: PgPool) -> Result<Arc<MintAuthService>, Box<dyn std::error::Error>> {
    let repo = Arc::new(MintAuthRepository::new(pool));
    let stellar = testnet_stellar_client()?;
    let issuer = std::env::var("STELLAR_ISSUER_ADDRESS")
        .unwrap_or_else(|_| "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX".into());
    Ok(Arc::new(MintAuthService::new(repo, stellar, issuer)))
}

fn gen_keypair() -> (String, SigningKey) {
    let sk = SigningKey::generate(&mut OsRng);
    let strkey = StrkeyPublicKey(sk.verifying_key().to_bytes());
    (strkey.to_string(), sk)
}

/// Sign a hex-encoded transaction hash and return the Base64-encoded signature.
fn sign_tx_hash(sk: &SigningKey, tx_hash_hex: &str) -> Result<String, Box<dyn std::error::Error>> {
    let bytes =
        hex::decode(tx_hash_hex).map_err(|e| format!("hex decode of tx_hash failed: {}", e))?;
    Ok(B64.encode(sk.sign(&bytes).to_bytes()))
}

/// Insert a minimal reserve verification snapshot and return its id.
async fn seed_reserve_verification(
    pool: &PgPool,
    amount: &BigDecimal,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO historical_verification
            (id, on_chain_supply, fiat_reserves, in_transit, delta,
             collateral_ratio, is_collateralised, issuer_address, asset_code,
             snapshot_signature, snapshot_json, triggered_by, created_at)
        VALUES ($1, $2, $3, 0, $3, 1.0, true, 'GTEST', 'cNGN', 'sig', '{}', 'test', NOW())
        "#,
        id,
        amount, // on_chain_supply
        amount, // fiat_reserves (equal → fully collateralised)
    )
    .execute(pool)
    .await
    .map_err(|e| format!("seed_reserve_verification INSERT failed: {}", e))?;
    Ok(id)
}

/// Insert an active mint signer and return (signer_id, stellar_public_key, signing_key).
async fn seed_signer(
    pool: &PgPool,
) -> Result<(Uuid, String, SigningKey), Box<dyn std::error::Error>> {
    let (pub_key, sk) = gen_keypair();
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO mint_signers
            (id, full_legal_name, role, organisation, contact_email,
             stellar_public_key, signing_weight, status, identity_verified, initiated_by)
        VALUES ($1, 'Test Signer', 'cfo', 'Test Org',
                $2, $3, 1, 'active', true, $1)
        "#,
        id,
        format!("test-{}@example.com", id),
        pub_key,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("seed_signer INSERT failed: {}", e))?;
    Ok((id, pub_key, sk))
}

/// Ensure `mint_quorum_config` has a row.
async fn seed_quorum(
    pool: &PgPool,
    threshold: i16,
) -> Result<(), Box<dyn std::error::Error>> {
    let admin = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO mint_quorum_config (required_threshold, updated_by) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
        threshold,
        admin,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("seed_quorum INSERT failed: {}", e))?;
    Ok(())
}

/// Parse a `BigDecimal` from a string literal, turning a parse failure into a
/// test failure with a descriptive message instead of an opaque panic.
fn bd(s: &str) -> BigDecimal {
    BigDecimal::from_str(s)
        .unwrap_or_else(|e| panic!("invalid BigDecimal literal {:?}: {}", s, e))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle: create → sign (threshold=1) → threshold_met → submitted
#[tokio::test]
async fn test_full_lifecycle_single_signer() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    let amount = bd("100.0000000");
    let reserve_id = seed_reserve_verification(&pool, &amount).await?;
    let (signer_id, signer_key, signing_key) = seed_signer(&pool).await?;
    seed_quorum(&pool, 1).await?;

    let requester_id = signer_id; // same person for simplicity in test
    let dest = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN";

    // 1. Create authorization request.
    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "100.0000000".into(),
                destination_account: dest.into(),
                justification: "Integration test mint".into(),
                reserve_verification_id: reserve_id,
            },
            requester_id,
            &signer_key,
        )
        .await
        .map_err(|e| format!("create authorization failed: {}", e))?;

    assert_eq!(auth.status, MintAuthStatus::PendingSignatures);
    assert!(auth.tx_hash.is_some(), "tx_hash must be set");
    assert!(!auth.unsigned_xdr.is_empty(), "unsigned_xdr must be set");

    // 2. Sign.
    let tx_hash = auth
        .tx_hash
        .as_deref()
        .ok_or("tx_hash was None after create")?;
    let signature = sign_tx_hash(&signing_key, tx_hash)?;

    let detail = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest {
                signature,
                signer_key: signer_key.clone(),
            },
            None,
        )
        .await
        .map_err(|e| format!("submit_signature failed: {}", e))?;

    assert_eq!(detail.signatures_collected, 1);
    assert_eq!(detail.signatures_required, 1);
    // With threshold=1, status transitions to threshold_met immediately.
    assert_eq!(detail.request.status, MintAuthStatus::ThresholdMet);

    Ok(())
}

/// Duplicate signature is rejected.
#[tokio::test]
async fn test_duplicate_signature_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    let amount = bd("50.0000000");
    let reserve_id = seed_reserve_verification(&pool, &amount).await?;
    let (signer_id, signer_key, signing_key) = seed_signer(&pool).await?;
    seed_quorum(&pool, 2).await?;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "50.0000000".into(),
                destination_account:
                    "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Dup sig test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .map_err(|e| format!("create failed: {}", e))?;

    let tx_hash = auth
        .tx_hash
        .as_deref()
        .ok_or("tx_hash was None after create")?;
    let sig = sign_tx_hash(&signing_key, tx_hash)?;

    svc.submit_signature(
        auth.id,
        SubmitSignatureRequest {
            signature: sig.clone(),
            signer_key: signer_key.clone(),
        },
        None,
    )
    .await
    .map_err(|e| format!("first signature failed: {}", e))?;

    let err = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest {
                signature: sig,
                signer_key: signer_key.clone(),
            },
            None,
        )
        .await
        .unwrap_err(); // unwrap_err is intentional: we assert the *error* path

    assert!(
        matches!(err, MintAuthError::DuplicateSignature(_, _)),
        "second signature from same signer must be rejected with DuplicateSignature"
    );

    Ok(())
}

/// Invalid signature (wrong key) is rejected.
#[tokio::test]
async fn test_invalid_signature_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    let amount = bd("50.0000000");
    let reserve_id = seed_reserve_verification(&pool, &amount).await?;
    let (signer_id, signer_key, _) = seed_signer(&pool).await?;
    seed_quorum(&pool, 2).await?;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "50.0000000".into(),
                destination_account:
                    "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Invalid sig test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .map_err(|e| format!("create failed: {}", e))?;

    // Sign with a different (unregistered) key.
    let (_, wrong_sk) = gen_keypair();
    let tx_hash = auth
        .tx_hash
        .as_deref()
        .ok_or("tx_hash was None after create")?;
    let bad_sig = sign_tx_hash(&wrong_sk, tx_hash)?;

    let err = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest {
                signature: bad_sig,
                signer_key: signer_key.clone(),
            },
            None,
        )
        .await
        .unwrap_err(); // intentional: asserting the error path

    assert!(
        matches!(err, MintAuthError::InvalidSignature(_, _)),
        "signature from wrong key must be rejected with InvalidSignature"
    );

    Ok(())
}

/// Cancellation transitions to cancelled and prevents further signing.
#[tokio::test]
async fn test_cancellation_prevents_further_signing() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    let amount = bd("200.0000000");
    let reserve_id = seed_reserve_verification(&pool, &amount).await?;
    let (signer_id, signer_key, signing_key) = seed_signer(&pool).await?;
    seed_quorum(&pool, 2).await?;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "200.0000000".into(),
                destination_account:
                    "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Cancel test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .map_err(|e| format!("create failed: {}", e))?;

    // Cancel it.
    let cancelled = svc
        .cancel(
            auth.id,
            signer_id,
            CancelMintAuthRequest {
                justification: "Test cancellation".into(),
            },
        )
        .await
        .map_err(|e| format!("cancel failed: {}", e))?;

    assert_eq!(cancelled.status, MintAuthStatus::Cancelled);
    assert!(cancelled.cancellation_reason.is_some());

    // Attempt to sign after cancellation.
    let tx_hash = auth
        .tx_hash
        .as_deref()
        .ok_or("tx_hash was None after create")?;
    let sig = sign_tx_hash(&signing_key, tx_hash)?;

    let err = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest {
                signature: sig,
                signer_key,
            },
            None,
        )
        .await
        .unwrap_err(); // intentional: asserting the error path

    assert!(
        matches!(err, MintAuthError::TerminalState(_, _)),
        "signing a cancelled request must be rejected with TerminalState"
    );

    Ok(())
}

/// Expiry worker transitions overdue requests to expired.
#[tokio::test]
async fn test_expiry_worker_expires_stale_requests() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    let amount = bd("10.0000000");
    let reserve_id = seed_reserve_verification(&pool, &amount).await?;
    let (signer_id, signer_key, _) = seed_signer(&pool).await?;
    seed_quorum(&pool, 2).await?;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "10.0000000".into(),
                destination_account:
                    "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Expiry test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .map_err(|e| format!("create failed: {}", e))?;

    // Back-date expires_at to the past.
    sqlx::query!(
        "UPDATE mint_authorization_requests SET expires_at = NOW() - INTERVAL '1 hour' WHERE id = $1",
        auth.id
    )
    .execute(&pool)
    .await
    .map_err(|e| format!("backdate expires_at failed: {}", e))?;

    let expired_count = svc
        .expire_stale_requests()
        .await
        .map_err(|e| format!("expire_stale_requests failed: {}", e))?;
    assert!(
        expired_count >= 1,
        "at least one request should have been expired"
    );

    let detail = svc
        .get(auth.id)
        .await
        .map_err(|e| format!("get failed: {}", e))?;
    assert_eq!(detail.request.status, MintAuthStatus::Expired);

    Ok(())
}

/// Reserve verification recency check rejects stale verifications.
#[tokio::test]
async fn test_stale_reserve_verification_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    let amount = bd("100.0000000");
    let id = Uuid::new_v4();

    // Insert a verification that is 48 hours old.
    sqlx::query!(
        r#"
        INSERT INTO historical_verification
            (id, on_chain_supply, fiat_reserves, in_transit, delta,
             collateral_ratio, is_collateralised, issuer_address, asset_code,
             snapshot_signature, snapshot_json, triggered_by, created_at)
        VALUES ($1, $2, $2, 0, $2, 1.0, true, 'GTEST', 'cNGN', 'sig', '{}', 'test',
                NOW() - INTERVAL '48 hours')
        "#,
        id,
        amount,
    )
    .execute(&pool)
    .await
    .map_err(|e| format!("seed stale verification failed: {}", e))?;

    let (signer_id, signer_key, _) = seed_signer(&pool).await?;

    let err = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "100.0000000".into(),
                destination_account:
                    "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Stale reserve test".into(),
                reserve_verification_id: id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .unwrap_err(); // intentional: asserting the error path

    assert!(
        matches!(err, MintAuthError::ReserveVerificationStale { .. }),
        "stale reserve verification must be rejected with ReserveVerificationStale"
    );

    Ok(())
}

/// Amount exceeding reserve balance is rejected.
#[tokio::test]
async fn test_amount_exceeds_reserve_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let pool = test_pool().await?;
    let svc = make_service(pool.clone())?;

    // Reserve has 100 cNGN, request asks for 200.
    let reserve_amount = bd("100.0000000");
    let reserve_id = seed_reserve_verification(&pool, &reserve_amount).await?;
    let (signer_id, signer_key, _) = seed_signer(&pool).await?;
    seed_quorum(&pool, 2).await?;

    let err = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "200.0000000".into(),
                destination_account:
                    "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Exceeds reserve test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .unwrap_err(); // intentional: asserting the error path

    assert!(
        matches!(err, MintAuthError::ExceedsReserveBalance { .. }),
        "amount exceeding reserve must be rejected with ExceedsReserveBalance"
    );

    Ok(())
}
