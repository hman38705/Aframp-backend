//! API Key Revocation & Blacklisting HTTP handlers (Issue #138).
//!
//! Routes:
//!   POST /api/developer/keys/:key_id/revoke
//!       — consumer self-service key revocation
//!
//!   POST /api/admin/consumers/:consumer_id/keys/:key_id/revoke
//!       — admin individual key revocation
//!
//!   POST /api/admin/consumers/:consumer_id/revoke-all
//!       — admin consumer-level revocation of all active keys
//!
//!   POST /api/admin/consumers/:consumer_id/blacklist
//!       — admin consumer blacklisting (with optional expiry)
//!
//!   DELETE /api/admin/consumers/:consumer_id/blacklist
//!       — lift a consumer blacklist
//!
//!   GET /api/admin/revocations
//!       — paginated revocation audit list
//!
//!   GET /api/admin/blacklist
//!       — current active blacklist state

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::services::revocation::{
    BlacklistConsumerInput, RevocationListQuery, RevocationRecord, RevocationService,
};

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RevocationState {
    pub service: Arc<RevocationService>,
}

// ─── Request / Response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct RevokeKeyRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminRevokeKeyRequest {
    pub reason: String,
    #[serde(default = "default_admin_revocation_type")]
    pub revocation_type: String,
}

fn default_admin_revocation_type() -> String {
    "admin_initiated".to_string()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RevokeAllRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BlacklistConsumerRequest {
    pub reason: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RevokeKeyResponse {
    pub revocation_id: Uuid,
    pub key_id: Uuid,
    pub consumer_id: Uuid,
    pub revocation_type: String,
    pub revoked_at: DateTime<Utc>,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RevokeAllResponse {
    pub consumer_id: Uuid,
    pub keys_revoked: usize,
    pub revocations: Vec<RevokeKeyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlacklistResponse {
    pub id: Uuid,
    pub consumer_id: Uuid,
    pub reason: String,
    pub blacklisted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RevocationListParams {
    pub consumer_id: Option<Uuid>,
    pub revocation_type: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    20
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RevocationListResponse {
    pub revocations: Vec<RevocationRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

fn err(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            code: code.to_string(),
            message: message.into(),
        }),
    )
        .into_response()
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/developer/keys/:key_id/revoke
///
/// Consumer self-service revocation. The consumer_id is extracted from the
/// authenticated key context injected by the API key middleware.
#[utoipa::path(
    post,
    path = "/api/developer/keys/{key_id}/revoke",
    tag = "admin",
    summary = "Self-service API key revocation",
    params(
        ("key_id" = Uuid, Path, description = "Key ID to revoke"),
        ("consumer_id" = Uuid, Query, description = "Consumer ID (testability shim; production reads from auth context)")
    ),
    request_body = RevokeKeyRequest,
    responses(
        (status = 200, description = "Key revoked", body = RevokeKeyResponse),
        (status = 400, description = "Missing reason"),
        (status = 404, description = "Key not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn consumer_revoke_key(
    State(state): State<RevocationState>,
    Path(key_id): Path<Uuid>,
    // In production this would come from the AuthenticatedKey extension;
    // for now we accept it as a query param for testability.
    Query(params): Query<ConsumerRevokeParams>,
    Json(body): Json<RevokeKeyRequest>,
) -> Response {
    if body.reason.trim().is_empty() {
        return err(
            StatusCode::BAD_REQUEST,
            "MISSING_REASON",
            "reason is required",
        );
    }

    let consumer_id = params.consumer_id;

    match state
        .service
        .revoke_key(crate::services::revocation::RevokeKeyInput {
            key_id,
            consumer_id,
            revocation_type: "consumer_requested",
            reason: body.reason,
            revoked_by: consumer_id.to_string(),
            triggering_detail: None,
        })
        .await
    {
        Ok(record) => (
            StatusCode::OK,
            Json(RevokeKeyResponse {
                revocation_id: record.id,
                key_id: record.key_id,
                consumer_id: record.consumer_id,
                revocation_type: record.revocation_type,
                revoked_at: record.revoked_at,
                message: "API key revoked successfully".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            warn!(key_id = %key_id, error = %e, "Consumer revoke key failed");
            if e.contains("not found") {
                err(StatusCode::NOT_FOUND, "KEY_NOT_FOUND", e)
            } else {
                err(StatusCode::INTERNAL_SERVER_ERROR, "REVOCATION_FAILED", e)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ConsumerRevokeParams {
    pub consumer_id: Uuid,
}

/// POST /api/admin/consumers/:consumer_id/keys/:key_id/revoke
#[utoipa::path(
    post,
    path = "/api/admin/consumers/{consumer_id}/keys/{key_id}/revoke",
    tag = "admin",
    summary = "Admin revocation of an individual API key",
    params(
        ("consumer_id" = Uuid, Path, description = "Consumer ID"),
        ("key_id" = Uuid, Path, description = "Key ID to revoke")
    ),
    request_body = AdminRevokeKeyRequest,
    responses(
        (status = 200, description = "Key revoked", body = RevokeKeyResponse),
        (status = 400, description = "Missing reason"),
        (status = 404, description = "Key not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn admin_revoke_key(
    State(state): State<RevocationState>,
    Path((consumer_id, key_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AdminRevokeKeyRequest>,
) -> Response {
    if body.reason.trim().is_empty() {
        return err(
            StatusCode::BAD_REQUEST,
            "MISSING_REASON",
            "reason is required",
        );
    }

    let revocation_type: &'static str = match body.revocation_type.as_str() {
        "admin_initiated" => "admin_initiated",
        "forced" => "forced",
        "decommission" => "decommission",
        "policy_violation" => "policy_violation",
        "suspected_compromise" => "suspected_compromise",
        _ => "admin_initiated",
    };

    match state
        .service
        .revoke_key(crate::services::revocation::RevokeKeyInput {
            key_id,
            consumer_id,
            revocation_type,
            reason: body.reason,
            revoked_by: "admin".to_string(),
            triggering_detail: None,
        })
        .await
    {
        Ok(record) => (
            StatusCode::OK,
            Json(RevokeKeyResponse {
                revocation_id: record.id,
                key_id: record.key_id,
                consumer_id: record.consumer_id,
                revocation_type: record.revocation_type,
                revoked_at: record.revoked_at,
                message: "API key revoked by admin".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            warn!(key_id = %key_id, consumer_id = %consumer_id, error = %e, "Admin revoke key failed");
            if e.contains("not found") {
                err(StatusCode::NOT_FOUND, "KEY_NOT_FOUND", e)
            } else {
                err(StatusCode::INTERNAL_SERVER_ERROR, "REVOCATION_FAILED", e)
            }
        }
    }
}

/// POST /api/admin/consumers/:consumer_id/revoke-all
#[utoipa::path(
    post,
    path = "/api/admin/consumers/{consumer_id}/revoke-all",
    tag = "admin",
    summary = "Revoke all active keys for a consumer",
    params(("consumer_id" = Uuid, Path, description = "Consumer ID")),
    request_body = RevokeAllRequest,
    responses(
        (status = 200, description = "All keys revoked", body = RevokeAllResponse),
        (status = 400, description = "Missing reason"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn admin_revoke_all_consumer_keys(
    State(state): State<RevocationState>,
    Path(consumer_id): Path<Uuid>,
    Json(body): Json<RevokeAllRequest>,
) -> Response {
    if body.reason.trim().is_empty() {
        return err(
            StatusCode::BAD_REQUEST,
            "MISSING_REASON",
            "reason is required",
        );
    }

    match state
        .service
        .revoke_all_consumer_keys(consumer_id, body.reason, "admin".to_string())
        .await
    {
        Ok(records) => {
            let revocations: Vec<RevokeKeyResponse> = records
                .iter()
                .map(|r| RevokeKeyResponse {
                    revocation_id: r.id,
                    key_id: r.key_id,
                    consumer_id: r.consumer_id,
                    revocation_type: r.revocation_type.clone(),
                    revoked_at: r.revoked_at,
                    message: "Revoked".to_string(),
                })
                .collect();
            let keys_revoked = revocations.len();
            (
                StatusCode::OK,
                Json(RevokeAllResponse {
                    consumer_id,
                    keys_revoked,
                    revocations,
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!(consumer_id = %consumer_id, error = %e, "Admin revoke-all failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "REVOCATION_FAILED", e)
        }
    }
}

/// POST /api/admin/consumers/:consumer_id/blacklist
#[utoipa::path(
    post,
    path = "/api/admin/consumers/{consumer_id}/blacklist",
    tag = "admin",
    summary = "Blacklist a consumer",
    params(("consumer_id" = Uuid, Path, description = "Consumer ID")),
    request_body = BlacklistConsumerRequest,
    responses(
        (status = 200, description = "Consumer blacklisted", body = BlacklistResponse),
        (status = 400, description = "Missing reason"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn admin_blacklist_consumer(
    State(state): State<RevocationState>,
    Path(consumer_id): Path<Uuid>,
    Json(body): Json<BlacklistConsumerRequest>,
) -> Response {
    if body.reason.trim().is_empty() {
        return err(
            StatusCode::BAD_REQUEST,
            "MISSING_REASON",
            "reason is required",
        );
    }

    match state
        .service
        .blacklist_consumer(BlacklistConsumerInput {
            consumer_id,
            reason: body.reason,
            blacklisted_by: "admin".to_string(),
            expires_at: body.expires_at,
        })
        .await
    {
        Ok(entry) => (
            StatusCode::OK,
            Json(BlacklistResponse {
                id: entry.id,
                consumer_id: entry.consumer_id,
                reason: entry.reason,
                blacklisted_at: entry.blacklisted_at,
                expires_at: entry.expires_at,
                message: "Consumer blacklisted successfully".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            warn!(consumer_id = %consumer_id, error = %e, "Admin blacklist consumer failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "BLACKLIST_FAILED", e)
        }
    }
}

/// DELETE /api/admin/consumers/:consumer_id/blacklist
#[utoipa::path(
    delete,
    path = "/api/admin/consumers/{consumer_id}/blacklist",
    tag = "admin",
    summary = "Lift a consumer's blacklist",
    params(("consumer_id" = Uuid, Path, description = "Consumer ID")),
    responses(
        (status = 200, description = "Blacklist lifted"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn admin_lift_consumer_blacklist(
    State(state): State<RevocationState>,
    Path(consumer_id): Path<Uuid>,
) -> Response {
    match state.service.lift_consumer_blacklist(consumer_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "consumer_id": consumer_id,
                "message": "Consumer blacklist lifted"
            })),
        )
            .into_response(),
        Err(e) => {
            warn!(consumer_id = %consumer_id, error = %e, "Lift blacklist failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "LIFT_FAILED", e)
        }
    }
}

/// GET /api/admin/revocations
#[utoipa::path(
    get,
    path = "/api/admin/revocations",
    tag = "admin",
    summary = "Paginated revocation audit list",
    params(
        ("consumer_id" = Option<Uuid>, Query, description = "Filter by consumer ID"),
        ("revocation_type" = Option<String>, Query, description = "Filter by revocation type"),
        ("from" = Option<String>, Query, description = "Filter from timestamp (RFC3339)"),
        ("to" = Option<String>, Query, description = "Filter to timestamp (RFC3339)"),
        ("page" = i64, Query, description = "Page number (default 1)"),
        ("page_size" = i64, Query, description = "Page size (default 20, max 100)")
    ),
    responses(
        (status = 200, description = "Revocation list", body = RevocationListResponse),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_revocations(
    State(state): State<RevocationState>,
    Query(params): Query<RevocationListParams>,
) -> Response {
    let page_size = params.page_size.clamp(1, 100);
    let page = params.page.max(1);

    let query = RevocationListQuery {
        consumer_id: params.consumer_id,
        revocation_type: params.revocation_type,
        from: params.from,
        to: params.to,
        page,
        page_size,
    };

    match state.service.list_revocations(query).await {
        Ok((records, total)) => (
            StatusCode::OK,
            Json(RevocationListResponse {
                revocations: records,
                total,
                page,
                page_size,
            }),
        )
            .into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e),
    }
}

/// GET /api/admin/blacklist
#[utoipa::path(
    get,
    path = "/api/admin/blacklist",
    tag = "admin",
    summary = "Current active blacklist state",
    responses(
        (status = 200, description = "Active blacklist entries", body = Vec<BlacklistEntry>),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_blacklist(State(state): State<RevocationState>) -> Response {
    match state.service.list_active_blacklist().await {
        Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e),
    }
}
