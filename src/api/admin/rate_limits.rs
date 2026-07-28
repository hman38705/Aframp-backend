//! Admin endpoints for per-consumer rate limit overrides (Issue #175 / #725).
//!
//! Routes:
//!   GET    /api/admin/consumers/:consumer_id/rate-limits              — effective limits + active overrides
//!   POST   /api/admin/consumers/:consumer_id/rate-limits              — create an override
//!   DELETE /api/admin/consumers/:consumer_id/rate-limits/:override_id — remove an override

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::database::consumer_rate_limit_repository::{ConsumerRateLimitRepository, LimitsJson};

// ─── State ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RateLimitAdminState {
    pub repo: Arc<ConsumerRateLimitRepository>,
}

// ─── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OverrideResponse {
    pub id: Uuid,
    pub consumer_id: Uuid,
    pub limits_json: LimitsJson,
    pub expiry_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ConsumerRateLimitsResponse {
    pub consumer_id: Uuid,
    /// Merged limits actually enforced (override, if any, else profile).
    pub effective_limits: Option<LimitsJson>,
    pub active_overrides: Vec<OverrideResponse>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOverrideRequest {
    pub limits_json: LimitsJson,
    #[serde(default)]
    pub expiry_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
            },
        }),
    )
        .into_response()
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /api/admin/consumers/:consumer_id/rate-limits
///
/// Returns the effective (override-or-profile-merged) rate limits for a
/// consumer plus the list of currently active overrides. Requires admin
/// authentication (enforced by the surrounding admin router layer).
pub async fn get_consumer_rate_limits(
    State(state): State<RateLimitAdminState>,
    Path(consumer_id): Path<Uuid>,
) -> Response {
    let effective_limits = match state.repo.get_effective_limits(consumer_id).await {
        Ok(limits) => limits,
        Err(e) => {
            error!("Failed to load effective rate limits for {}: {}", consumer_id, e);
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to load rate limits",
            );
        }
    };

    let overrides = match state.repo.list_overrides(consumer_id).await {
        Ok(overrides) => overrides,
        Err(e) => {
            error!("Failed to list rate limit overrides for {}: {}", consumer_id, e);
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to load rate limit overrides",
            );
        }
    };

    Json(ConsumerRateLimitsResponse {
        consumer_id,
        effective_limits,
        active_overrides: overrides
            .into_iter()
            .map(|o| OverrideResponse {
                id: o.id,
                consumer_id: o.consumer_id,
                limits_json: o.limits_json,
                expiry_at: o.expiry_at,
                reason: o.reason,
                created_at: o.created_at,
            })
            .collect(),
    })
    .into_response()
}

/// POST /api/admin/consumers/:consumer_id/rate-limits
///
/// Creates a rate-limit override for a consumer — takes precedence over
/// their consumer-type profile until `expiry_at` (or forever if unset).
pub async fn create_consumer_rate_limit_override(
    State(state): State<RateLimitAdminState>,
    Path(consumer_id): Path<Uuid>,
    Json(body): Json<CreateOverrideRequest>,
) -> Response {
    if !body.limits_json.is_object() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "INVALID_LIMITS_JSON",
            "limits_json must be an object of dimension -> {limit, window_secs}",
        );
    }

    match state
        .repo
        .create_override(
            consumer_id,
            &body.limits_json,
            body.expiry_at,
            None,
            body.reason,
        )
        .await
    {
        Ok(o) => {
            info!(consumer_id = %consumer_id, override_id = %o.id, "Admin created rate limit override");
            (
                StatusCode::CREATED,
                Json(OverrideResponse {
                    id: o.id,
                    consumer_id: o.consumer_id,
                    limits_json: o.limits_json,
                    expiry_at: o.expiry_at,
                    reason: o.reason,
                    created_at: o.created_at,
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to create rate limit override for {}: {}", consumer_id, e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to create rate limit override",
            )
        }
    }
}

/// DELETE /api/admin/consumers/:consumer_id/rate-limits/:override_id
///
/// Removes an override, reverting the consumer to their consumer-type
/// profile limits.
pub async fn delete_consumer_rate_limit_override(
    State(state): State<RateLimitAdminState>,
    Path((consumer_id, override_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match state.repo.delete_override(override_id).await {
        Ok(true) => {
            info!(consumer_id = %consumer_id, override_id = %override_id, "Admin deleted rate limit override");
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => error_response(
            StatusCode::NOT_FOUND,
            "OVERRIDE_NOT_FOUND",
            "No override found with that id",
        ),
        Err(e) => {
            error!("Failed to delete rate limit override {}: {}", override_id, e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to delete rate limit override",
            )
        }
    }
}
