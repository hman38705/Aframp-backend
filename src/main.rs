use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use std::{net::SocketAddr, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    webhook_secret: Arc<String>,
    system_wallet: Arc<String>,
    issuer: Arc<String>,
}

#[derive(Deserialize)]
struct QuoteRequest {
    direction: String,
    wallet_address: String,
    amount_kobo: i64,
}

#[derive(Serialize)]
struct QuoteResponse {
    quote_id: Uuid,
    amount_kobo: i64,
    cngn_stroops: i64,
    expires_at: String,
}

#[derive(Deserialize)]
struct InitiateRequest {
    quote_id: Uuid,
    wallet_address: String,
    payment_provider: Option<String>,
    bank_code: Option<String>,
    account_number: Option<String>,
}

#[derive(Deserialize)]
struct Webhook {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    transaction_id: Uuid,
}

#[derive(Serialize, FromRow)]
struct Transaction {
    id: Uuid,
    direction: String,
    wallet_address: String,
    amount_kobo: i64,
    cngn_stroops: i64,
    status: String,
    payment_reference: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Serialize)]
struct StartResponse {
    transaction_id: Uuid,
    status: String,
    payment_reference: Option<String>,
    deposit_instructions: Option<DepositInstructions>,
}

#[derive(Serialize)]
struct DepositInstructions {
    destination: String,
    asset: String,
    issuer: String,
    amount_stroops: i64,
    memo: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ErrorResponse>)>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let database_url = required_env("DATABASE_URL")?;
    let state = AppState {
        db: PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?,
        webhook_secret: Arc::new(required_env("WEBHOOK_SECRET")?),
        system_wallet: Arc::new(required_env("SYSTEM_WALLET_ADDRESS")?),
        issuer: Arc::new(required_env("CNGN_ISSUER_ADDRESS")?),
    };
    let app = Router::new()
        .route("/health", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/quotes", post(create_quote))
        .route("/v1/onramps", post(start_onramp))
        .route("/v1/offramps", post(start_offramp))
        .route("/v1/transactions/{id}", get(get_transaction))
        .route("/v1/webhooks/{provider}", post(payment_webhook))
        .with_state(state);
    let address: SocketAddr = std::env::var("APP_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()?;
    axum::serve(tokio::net::TcpListener::bind(address).await?, app).await?;
    Ok(())
}

async fn create_quote(
    State(state): State<AppState>,
    Json(req): Json<QuoteRequest>,
) -> ApiResult<Json<QuoteResponse>> {
    if !matches!(req.direction.as_str(), "onramp" | "offramp")
        || req.wallet_address.is_empty()
        || req.amount_kobo <= 0
    {
        return Err(bad_request(
            "direction, wallet_address, and positive amount_kobo are required",
        ));
    }
    // V1 assumes a 1 NGN = 1 cNGN rate. Store cNGN in 7-decimal Stellar stroops.
    let cngn_stroops = req
        .amount_kobo
        .checked_mul(100_000)
        .ok_or_else(|| bad_request("amount is too large"))?;
    let quote_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::minutes(5);
    sqlx::query("INSERT INTO quotes (id, direction, wallet_address, amount_kobo, cngn_stroops, expires_at) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(quote_id).bind(&req.direction).bind(&req.wallet_address).bind(req.amount_kobo).bind(cngn_stroops).bind(expires_at)
        .execute(&state.db).await.map_err(internal)?;
    Ok(Json(QuoteResponse {
        quote_id,
        amount_kobo: req.amount_kobo,
        cngn_stroops,
        expires_at: expires_at.to_rfc3339(),
    }))
}

async fn start_onramp(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<InitiateRequest>,
) -> ApiResult<Json<StartResponse>> {
    let provider = req
        .payment_provider
        .clone()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| bad_request("payment_provider is required"))?;
    let tx = start_transaction(&state.db, &headers, req, "onramp", Some(&provider)).await?;
    Ok(Json(StartResponse {
        transaction_id: tx.id,
        status: tx.status,
        payment_reference: tx.payment_reference,
        deposit_instructions: None,
    }))
}

async fn start_offramp(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<InitiateRequest>,
) -> ApiResult<Json<StartResponse>> {
    if req.bank_code.as_deref().unwrap_or("").is_empty()
        || req.account_number.as_deref().map(str::len) != Some(10)
    {
        return Err(bad_request(
            "bank_code and a 10-digit account_number are required",
        ));
    }
    let tx = start_transaction(&state.db, &headers, req, "offramp", None).await?;
    Ok(Json(StartResponse {
        transaction_id: tx.id,
        status: tx.status,
        payment_reference: None,
        deposit_instructions: Some(DepositInstructions {
            destination: (*state.system_wallet).clone(),
            asset: "cNGN".into(),
            issuer: (*state.issuer).clone(),
            amount_stroops: tx.cngn_stroops,
            memo: format!("OFFRAMP-{}", &tx.id.simple().to_string()[..12]),
        }),
    }))
}

async fn start_transaction(
    db: &PgPool,
    headers: &HeaderMap,
    req: InitiateRequest,
    direction: &str,
    provider: Option<&str>,
) -> ApiResult<Transaction> {
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| bad_request("Idempotency-Key header is required"))?;
    if let Some(tx) = transaction_by_key(db, key).await.map_err(internal)? {
        return Ok(tx);
    }
    let payment_reference = provider.map(|_| format!("onr_{}", Uuid::new_v4().simple()));
    sqlx::query_as::<_, Transaction>("WITH quote AS (UPDATE quotes SET consumed_at = now() WHERE id = $1 AND direction = $2 AND wallet_address = $3 AND consumed_at IS NULL AND expires_at > now() RETURNING *) INSERT INTO transactions (direction, quote_id, wallet_address, amount_kobo, cngn_stroops, status, idempotency_key, payment_provider, payment_reference) SELECT $2, id, wallet_address, amount_kobo, cngn_stroops, CASE WHEN $2 = 'onramp' THEN 'awaiting_payment' ELSE 'awaiting_token' END, $4, $5, $6 FROM quote RETURNING id, direction, wallet_address, amount_kobo, cngn_stroops, status, payment_reference, created_at, updated_at")
        .bind(req.quote_id).bind(direction).bind(&req.wallet_address).bind(key).bind(provider).bind(payment_reference)
        .fetch_optional(db).await.map_err(internal)?.ok_or_else(|| conflict("quote is expired, consumed, or does not belong to this wallet"))
}

async fn get_transaction(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> ApiResult<Json<Transaction>> {
    transaction_by_id(&state.db, id)
        .await
        .map_err(internal)?
        .map(Json)
        .ok_or_else(|| not_found("transaction not found"))
}

async fn payment_webhook(
    Path(provider): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<StatusCode> {
    let signature = headers
        .get("x-webhook-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| unauthorized())?;
    if !valid_signature(&state.webhook_secret, &body, signature) {
        return Err(unauthorized());
    }
    let event: Webhook =
        serde_json::from_slice(&body).map_err(|_| bad_request("invalid webhook payload"))?;
    let inserted = sqlx::query("INSERT INTO webhook_events (provider, external_id, transaction_id, payload) VALUES ($1,$2,$3,$4) ON CONFLICT (provider, external_id) DO NOTHING")
        .bind(provider).bind(&event.id).bind(event.transaction_id).bind(serde_json::from_slice::<serde_json::Value>(&body).map_err(|_| bad_request("invalid webhook payload"))?)
        .execute(&state.db).await.map_err(internal)?.rows_affected();
    if inserted == 1 && event.event_type == "payment.success" {
        // Replace this state update with a Stellar mint submission before production.
        sqlx::query("UPDATE transactions SET status = 'payment_confirmed', updated_at = now() WHERE id = $1 AND direction = 'onramp' AND status = 'awaiting_payment'")
            .bind(event.transaction_id).execute(&state.db).await.map_err(internal)?;
    }
    Ok(StatusCode::OK)
}

async fn transaction_by_key(db: &PgPool, key: &str) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as("SELECT id, direction, wallet_address, amount_kobo, cngn_stroops, status, payment_reference, created_at, updated_at FROM transactions WHERE idempotency_key = $1").bind(key).fetch_optional(db).await
}
async fn transaction_by_id(db: &PgPool, id: Uuid) -> Result<Option<Transaction>, sqlx::Error> {
    sqlx::query_as("SELECT id, direction, wallet_address, amount_kobo, cngn_stroops, status, payment_reference, created_at, updated_at FROM transactions WHERE id = $1").bind(id).fetch_optional(db).await
}
fn valid_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let Ok(bytes) = hex::decode(signature) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&bytes).is_ok()
}
fn required_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is required"))
}
fn response(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}
fn bad_request(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    response(StatusCode::BAD_REQUEST, message)
}
fn conflict(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    response(StatusCode::CONFLICT, message)
}
fn not_found(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    response(StatusCode::NOT_FOUND, message)
}
fn unauthorized() -> (StatusCode, Json<ErrorResponse>) {
    response(StatusCode::UNAUTHORIZED, "invalid webhook signature")
}
fn internal(_: sqlx::Error) -> (StatusCode, Json<ErrorResponse>) {
    response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}
