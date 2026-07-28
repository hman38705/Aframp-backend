//! OpenAPI / Swagger documentation.
//!
//! Routes:
//!   GET /docs              — Swagger UI
//!   GET /docs/openapi.json — Raw OpenAPI 3.0 JSON
//!
//! In production the full API schema (including internal admin endpoint
//! paths) is sensitive, so both routes require a static bearer token
//! (`SWAGGER_API_KEY`) in that environment. Dev/staging remain open. Set
//! `SWAGGER_ENABLED=false` to disable Swagger entirely, in any environment.

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Router,
};
use serde::{Deserialize, Serialize};
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi, ToSchema,
};
use utoipa_swagger_ui::SwaggerUi;

// ─── Security Modifier ───────────────────────────────────────────────────────

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearerAuth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

// ─── Shared Schemas ──────────────────────────────────────────────────────────

/// Standard error response returned by all API endpoints on failure.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorResponse {
    pub error: ApiErrorDetail,
}

/// Detail block inside an API error response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorDetail {
    /// Machine-readable error code e.g. "TRUSTLINE_REQUIRED"
    pub code: String,
    /// Human-readable error description
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Transaction status used across onramp, offramp, and bill payment flows.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Refunded,
}

/// Supported blockchain chains.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Stellar,
}

/// Pagination query parameters.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

// ─── Onramp Schemas ──────────────────────────────────────────────────────────

/// Request body for POST /api/onramp/quote
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OnrampQuoteRequest {
    pub amount_ngn: String,
    pub wallet_address: String,
    pub provider: String,
    pub chain: Chain,
}

/// Fee breakdown inside a quote response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QuoteFeeSummary {
    pub platform_fee_ngn: String,
    pub provider_fee_ngn: String,
    pub total_fee_ngn: String,
    pub platform_fee_pct: String,
    pub provider_fee_pct: String,
}

/// Response for POST /api/onramp/quote
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OnrampQuoteResponse {
    pub quote_id: String,
    pub expires_at: String,
    pub expires_in_seconds: i64,
    pub amount_ngn: String,
    pub amount_cngn: String,
    pub fees: QuoteFeeSummary,
    pub trustline_required: bool,
    pub liquidity_available: bool,
}

/// Request body for POST /api/onramp/initiate
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OnrampInitiateRequest {
    pub quote_id: String,
    pub wallet_address: String,
}

/// Response for POST /api/onramp/initiate
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OnrampInitiateResponse {
    pub transaction_id: String,
    pub status: TransactionStatus,
    pub payment_reference: String,
    pub amount_ngn: String,
}

/// Response for GET /api/onramp/status/:tx_id
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OnrampStatusResponse {
    pub transaction_id: String,
    pub status: TransactionStatus,
    pub amount_ngn: String,
    pub amount_cngn: String,
    pub wallet_address: String,
    pub stellar_tx_hash: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Offramp Schemas ─────────────────────────────────────────────────────────

/// Request body for POST /api/offramp/quote
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OfframpQuoteRequest {
    pub amount_cngn: String,
    pub wallet_address: String,
    pub provider: String,
}

/// Response for POST /api/offramp/quote
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OfframpQuoteResponse {
    pub quote_id: String,
    pub expires_at: String,
    pub expires_in_seconds: i64,
    pub amount_cngn: String,
    pub fees: QuoteFeeSummary,
    pub amount_ngn_after_fees: String,
    pub collection_address: String,
}

/// Request body for POST /api/offramp/initiate
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OfframpInitiateRequest {
    pub quote_id: String,
    pub wallet_address: String,
    pub bank_account_number: Option<String>,
    pub bank_code: Option<String>,
}

/// Response for POST /api/offramp/initiate
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OfframpInitiateResponse {
    pub transaction_id: String,
    pub status: TransactionStatus,
    pub collection_address: String,
    pub amount_cngn: String,
    pub memo: String,
}

/// Response for GET /api/offramp/status/:tx_id
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OfframpStatusResponse {
    pub transaction_id: String,
    pub status: TransactionStatus,
    pub amount_cngn: String,
    pub amount_ngn: String,
    pub stellar_tx_hash: Option<String>,
    pub payout_reference: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Bills Schemas ───────────────────────────────────────────────────────────

/// A single bill payment provider.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BillProvider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub status: String,
}

/// Response for GET /api/bills/providers
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BillProvidersResponse {
    pub providers: Vec<BillProvider>,
    pub total: u32,
}

/// Request body for POST /api/bills/pay
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BillPayRequest {
    pub provider_id: String,
    pub customer_id: String,
    pub amount_ngn: String,
    pub wallet_address: String,
}

/// Response for POST /api/bills/pay
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BillPayResponse {
    pub transaction_id: String,
    pub status: TransactionStatus,
    pub reference: String,
    pub amount_ngn: String,
    pub amount_cngn: String,
}

// ─── Rates Schemas ───────────────────────────────────────────────────────────

/// A single exchange rate entry.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RateEntry {
    pub pair: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: String,
    pub inverse_rate: String,
    pub last_updated: String,
    pub source: String,
}

// ─── Fees Schemas ────────────────────────────────────────────────────────────

/// Fee detail for a single flow direction.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeeDetail {
    pub platform_fee_pct: String,
    pub provider_fee_pct: String,
    pub minimum_fee_ngn: String,
}

/// Response for GET /api/fees
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeeStructureResponse {
    pub onramp: FeeDetail,
    pub offramp: FeeDetail,
}

// ─── Wallet Schemas ──────────────────────────────────────────────────────────

/// Response for GET /api/wallet/balance
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WalletBalanceResponse {
    pub wallet_address: String,
    pub xlm_balance: String,
    pub cngn_balance: String,
    pub has_cngn_trustline: bool,
    pub last_updated: String,
}

// ─── Batch Schemas ───────────────────────────────────────────────────────────

/// A single transfer item within a batch cNGN transfer request.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CngnTransferItem {
    pub destination_wallet: String,
    pub amount_cngn: String,
    pub memo: Option<String>,
}

/// Request body for POST /api/batch/cngn-transfer
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchCngnTransferRequest {
    pub source_wallet: String,
    pub transfers: Vec<CngnTransferItem>,
}

/// A single fiat payout item within a batch fiat payout request.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FiatPayoutItem {
    pub bank_account_number: String,
    pub bank_code: String,
    pub amount_ngn: String,
    pub reference: Option<String>,
}

/// Request body for POST /api/batch/fiat-payout
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchFiatPayoutRequest {
    pub payouts: Vec<FiatPayoutItem>,
}

/// Response for POST /api/batch/cngn-transfer and POST /api/batch/fiat-payout
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchCreateResponse {
    pub batch_id: String,
    pub status: String,
    pub total_items: u32,
    pub created_at: String,
}

/// Status of an individual item within a batch.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchItemStatus {
    pub item_id: String,
    pub status: String,
    pub destination: String,
    pub amount: String,
    pub stellar_tx_hash: Option<String>,
    pub failure_reason: Option<String>,
}

/// Response for GET /api/batch/:batch_id
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchStatusResponse {
    pub batch_id: String,
    pub batch_type: String,
    pub status: String,
    pub total_count: u32,
    pub success_count: u32,
    pub failed_count: u32,
    pub pending_count: u32,
    pub items: Vec<BatchItemStatus>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

// ─── Admin / Scopes Schemas ──────────────────────────────────────────────────

/// A platform scope definition.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ScopeDefinition {
    pub name: String,
    pub description: String,
    pub category: String,
    pub applicable_consumer_types: Vec<String>,
}

/// Response for GET /api/admin/scopes
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ScopesListResponse {
    pub scopes: Vec<ScopeDefinition>,
    pub total: u32,
}

/// Scopes currently assigned to an API key.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct KeyScopesResponse {
    pub key_id: String,
    pub consumer_id: String,
    pub consumer_type: String,
    pub scopes: Vec<String>,
}

/// Request body for PATCH /api/admin/consumers/:consumer_id/keys/:key_id/scopes
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateScopesRequest {
    /// Complete scope list to assign — replaces all existing scopes for this key.
    pub scopes: Vec<String>,
}

/// Signed Proof-of-Reserves response returned by `GET /v1/public/transparency`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransparencyResponseSchema {
    /// Total cNGN in circulation.
    pub total_supply: String,
    /// Total NGN reserves held.
    pub total_reserves: String,
    /// Ratio of reserves to supply (1.0 = fully backed).
    pub collateral_ratio: String,
    /// ISO-8601 timestamp of the most recent snapshot.
    pub last_updated_timestamp: String,
    /// URL to the third-party audit report, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_link: Option<String>,
    /// Hex-encoded Ed25519 signature over the canonical payload.
    pub signature: String,
    /// Hex-encoded Ed25519 public key used to produce `signature`.
    pub signing_key: String,
}

/// A single historical data point for `GET /v1/public/transparency/history`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReserveDataPointSchema {
    pub total_supply: String,
    pub total_reserves: String,
    pub collateral_ratio: String,
    pub timestamp: String,
}

/// Response for `GET /v1/public/transparency/history`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TransparencyHistorySchema {
    pub period_days: u32,
    pub data_points: Vec<ReserveDataPointSchema>,
}

// ─── OpenAPI Document ────────────────────────────────────────────────────────

/// Root OpenAPI document generated from all annotated schemas and paths.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Aframp API",
        version = "0.1.0",
        description = "
# Aframp API

Platform API for NGN ↔ cNGN onramp/offramp, bill payments, exchange rates, and wallet management.

## Authentication

All authenticated endpoints require a JWT bearer token:

```
Authorization: Bearer <your_jwt_token>
```

## Quick Start

1. Request a quote: `POST /api/onramp/quote`
2. Initiate the transaction: `POST /api/onramp/initiate`
3. Poll status: `GET /api/onramp/status/:tx_id`

Your Stellar wallet must have an active cNGN trustline before receiving cNGN.
See the cNGN Integration Guide in `/docs/cngn/` for setup instructions.

## Rate Limiting

Every response carries your current quota for the endpoint you called:

| Header                | Meaning                                              |
|------------------------|-------------------------------------------------------|
| `X-RateLimit-Limit`     | Requests allowed in the current window                |
| `X-RateLimit-Remaining` | Requests left in the current window                   |
| `X-RateLimit-Reset`     | Unix timestamp (seconds) when the window resets        |
| `X-RateLimit-Used`      | Requests already consumed in the current window        |

Exceeding the limit returns `429 Too Many Requests` with an additional
`Retry-After` header (seconds until the window resets).

Endpoints are grouped into sensitivity tiers, each with its own default
per-IP ceiling (requests/minute): `CRITICAL` (10) for fund-movement
endpoints such as mint and redemption, `FINANCIAL` (60) for onramp/offramp
initiation, `STANDARD` (300) for general authenticated endpoints, and
`PUBLIC` (1000) for read-only endpoints such as `/api/rates`.

API-key consumers may additionally be subject to a per-consumer profile
(by consumer type: mobile_client, partner, microservice, admin) checked
across four dimensions — global, endpoint sensitivity, transaction type,
and IP — the most restrictive of which determines the response. Admins
manage per-consumer overrides (which take precedence over the profile
until they expire) via:

- `GET /api/admin/consumers/:consumer_id/rate-limits` — effective limits + active overrides
- `POST /api/admin/consumers/:consumer_id/rate-limits` — create an override (`limits_json`, optional `expiry_at`, `reason`)
- `DELETE /api/admin/consumers/:consumer_id/rate-limits/:override_id` — remove an override
",
        contact(name = "Aframp Engineering"),
        license(name = "Proprietary")
    ),
    components(schemas(
        ApiErrorResponse,
        ApiErrorDetail,
        TransactionStatus,
        Chain,
        PaginationQuery,
        QuoteFeeSummary,
        OnrampQuoteRequest,
        OnrampQuoteResponse,
        OnrampInitiateRequest,
        OnrampInitiateResponse,
        OnrampStatusResponse,
        OfframpQuoteRequest,
        OfframpQuoteResponse,
        OfframpInitiateRequest,
        OfframpInitiateResponse,
        OfframpStatusResponse,
        BillProvider,
        BillProvidersResponse,
        BillPayRequest,
        BillPayResponse,
        RateEntry,
        FeeDetail,
        FeeStructureResponse,
        WalletBalanceResponse,
        CngnTransferItem,
        BatchCngnTransferRequest,
        FiatPayoutItem,
        BatchFiatPayoutRequest,
        BatchCreateResponse,
        BatchItemStatus,
        BatchStatusResponse,
        ScopeDefinition,
        ScopesListResponse,
        KeyScopesResponse,
        UpdateScopesRequest,
        TransparencyResponseSchema,
        ReserveDataPointSchema,
        TransparencyHistorySchema,
    )),
    paths(
        crate::partner::handlers::register_partner,
        crate::partner::handlers::get_partner,
        crate::partner::handlers::provision_credential,
        crate::partner::handlers::revoke_credential,
        crate::partner::handlers::validate_partner,
        crate::partner::handlers::promote_to_production,
        crate::partner::handlers::list_deprecations,
        crate::partner::handlers::partner_me,
    ),
    tags(
        (name = "onramp", description = "NGN to cNGN conversion (fiat to crypto)"),
        (name = "offramp", description = "cNGN to NGN conversion (crypto to fiat)"),
        (name = "bills", description = "Bill payment services"),
        (name = "rates", description = "Exchange rates and fees"),
        (name = "wallet", description = "Wallet balance and trustline management"),
        (name = "batch", description = "Batch transaction processing"),
        (name = "admin", description = "Administrative endpoints — require admin authentication"),
        (name = "transparency", description = "Public Proof-of-Reserves data feed for aggregators"),
        (name = "partner", description = "Partner Integration Framework — self-service onboarding, credential management, and API versioning"),
    ),
    modifiers(&SecurityAddon),
    servers(
        (url = "/", description = "Current environment"),
        (url = "http://localhost:8000", description = "Local development"),
    )
)]
pub struct ApiDoc;

// ─── Environment / access gating ─────────────────────────────────────────────

/// Current deployment environment, checking `ENVIRONMENT` then falling back
/// to `APP_ENV` — the same precedence used elsewhere (e.g.
/// `middleware::security`, `middleware::cors`).
fn current_environment() -> String {
    std::env::var("ENVIRONMENT")
        .or_else(|_| std::env::var("APP_ENV"))
        .unwrap_or_else(|_| "development".to_string())
        .to_lowercase()
}

fn is_production() -> bool {
    current_environment() == "production"
}

/// `SWAGGER_ENABLED=false` disables Swagger entirely, in any environment.
/// Defaults to enabled.
fn swagger_enabled() -> bool {
    std::env::var("SWAGGER_ENABLED")
        .map(|v| v.to_lowercase() != "false")
        .unwrap_or(true)
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Extract a bearer/API-key style credential from the request: either a
/// standard `Authorization: Bearer <token>` header, or `X-Swagger-Key:
/// <token>` for callers that can't easily set an Authorization header
/// (e.g. a browser navigating to `/docs` directly).
fn extract_swagger_token(req: &Request) -> Option<String> {
    if let Some(v) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(v.to_string());
    }
    req.headers()
        .get("x-swagger-key")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
}

/// Gate `/docs` and `/docs/openapi.json` behind `SWAGGER_API_KEY` in
/// production. Fails closed: if `SWAGGER_API_KEY` isn't configured, every
/// request is rejected rather than falling open.
async fn swagger_auth_middleware(req: Request, next: Next) -> Response {
    let expected = std::env::var("SWAGGER_API_KEY").unwrap_or_default();
    let provided = extract_swagger_token(&req);

    let authorized = !expected.is_empty()
        && provided
            .as_deref()
            .is_some_and(|p| constant_time_eq(p, &expected));

    if authorized {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer realm=\"swagger\"")],
            "Unauthorized",
        )
            .into_response()
    }
}

// ─── Route Builder ───────────────────────────────────────────────────────────

/// Build the Axum router for OpenAPI documentation endpoints.
///
/// - `GET /docs` (Swagger UI) and `GET /docs/openapi.json` are always mounted
///   together, unless `SWAGGER_ENABLED=false`.
/// - In production both routes require `Authorization: Bearer <SWAGGER_API_KEY>`
///   (or an `X-Swagger-Key` header). Dev/staging are unauthenticated.
pub fn openapi_routes() -> Router {
    if !swagger_enabled() {
        return Router::new();
    }

    let openapi = ApiDoc::openapi();
    let docs_router = Router::new().merge(SwaggerUi::new("/docs").url("/docs/openapi.json", openapi));

    if is_production() {
        docs_router.layer(axum::middleware::from_fn(swagger_auth_middleware))
    } else {
        docs_router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::ServiceExt;

    // Env vars are process-global and these tests mutate them directly
    // (matching the existing convention in middleware::cors's tests) — each
    // test resets everything it touches at the top, so ordering doesn't matter
    // even though `cargo test` runs them on separate threads within this binary.
    fn reset_env() {
        std::env::remove_var("ENVIRONMENT");
        std::env::remove_var("APP_ENV");
        std::env::remove_var("SWAGGER_API_KEY");
        std::env::remove_var("SWAGGER_ENABLED");
    }

    async fn get(app: Router, uri: &str, auth_header: Option<&str>) -> StatusCode {
        let mut builder = Request::builder().uri(uri);
        if let Some(h) = auth_header {
            builder = builder.header(header::AUTHORIZATION, h);
        }
        let response = app.oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
        response.status()
    }

    #[tokio::test]
    async fn test_production_without_token_returns_401() {
        reset_env();
        std::env::set_var("ENVIRONMENT", "production");
        std::env::set_var("SWAGGER_API_KEY", "secret-token");

        let status = get(openapi_routes(), "/docs/openapi.json", None).await;

        reset_env();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_production_with_correct_token_is_authorized() {
        reset_env();
        std::env::set_var("ENVIRONMENT", "production");
        std::env::set_var("SWAGGER_API_KEY", "secret-token");

        let status = get(
            openapi_routes(),
            "/docs/openapi.json",
            Some("Bearer secret-token"),
        )
        .await;

        reset_env();
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_production_with_wrong_token_returns_401() {
        reset_env();
        std::env::set_var("ENVIRONMENT", "production");
        std::env::set_var("SWAGGER_API_KEY", "secret-token");

        let status = get(
            openapi_routes(),
            "/docs/openapi.json",
            Some("Bearer wrong-token"),
        )
        .await;

        reset_env();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_production_without_configured_key_fails_closed() {
        reset_env();
        std::env::set_var("ENVIRONMENT", "production");
        // SWAGGER_API_KEY intentionally left unset.

        let status = get(
            openapi_routes(),
            "/docs/openapi.json",
            Some("Bearer anything"),
        )
        .await;

        reset_env();
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "must fail closed when no key is configured"
        );
    }

    #[tokio::test]
    async fn test_development_is_unauthenticated() {
        reset_env();
        std::env::set_var("ENVIRONMENT", "development");

        let status = get(openapi_routes(), "/docs/openapi.json", None).await;

        reset_env();
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_swagger_disabled_returns_404_regardless_of_environment() {
        reset_env();
        std::env::set_var("ENVIRONMENT", "production");
        std::env::set_var("SWAGGER_API_KEY", "secret-token");
        std::env::set_var("SWAGGER_ENABLED", "false");

        let status = get(
            openapi_routes(),
            "/docs/openapi.json",
            Some("Bearer secret-token"),
        )
        .await;

        reset_env();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
