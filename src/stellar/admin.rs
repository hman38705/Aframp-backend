/// Admin endpoints for Stellar submission channel management
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::stellar::models::{BatchEnvelopeRequest, BatchSubmissionResult, SubmissionMetrics};
use crate::stellar::submission::StellarSubmissionEngine;

/// Admin state for stellar routes
#[derive(Clone)]
pub struct StellarAdminState {
    pub pool: PgPool,
    pub submission_engine: std::sync::Arc<StellarSubmissionEngine>,
}

/// Channel status response
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelStatusResponse {
    pub channel_id: String,
    pub index: i32,
    pub account_id: String,
    pub balance_xlm: Decimal,
    pub current_sequence: i64,
    pub reserved_sequence: i64,
    pub in_flight_transactions: i64,
    pub total_submitted: u64,
    pub total_successful: u64,
    pub total_failed: u64,
    pub consecutive_failures: u32,
    pub is_circuit_broken: bool,
    pub status: String,
}

/// Channel top-up request
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelTopUpRequest {
    pub channel_index: i32,
    pub amount_xlm: Decimal,
    pub description: Option<String>,
}

/// Admin response
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Forensics query parameters
#[derive(Debug, Deserialize)]
pub struct ForensicsQuery {
    pub limit: Option<i64>,
    pub error_code: Option<String>,
}

/// Forensic failure row returned from the DB
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ForensicFailureRow {
    pub id: Uuid,
    pub queue_id: Option<Uuid>,
    pub tx_log_id: Option<Uuid>,
    pub issuer_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub error_code: Option<String>,
    pub error_reason: Option<String>,
    pub horizon_status: Option<String>,
    pub retryable: Option<bool>,
    pub occurred_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Get all submission channels status
async fn get_channels(
    State(state): State<StellarAdminState>,
) -> Result<Json<AdminResponse<Vec<ChannelStatusResponse>>>, AdminError> {
    let stats = state.submission_engine.get_pool_stats().await?;

    let channels: Vec<ChannelStatusResponse> = stats
        .iter()
        .map(|stat| {
            let balance = stat["balance_xlm"].as_f64().unwrap_or(0.0);
            let status = if stat["is_circuit_broken"].as_bool().unwrap_or(false) {
                "circuit_broken".to_string()
            } else if stat["in_flight"].as_i64().unwrap_or(0) > 900 {
                "exhausted".to_string()
            } else {
                "healthy".to_string()
            };

            ChannelStatusResponse {
                channel_id: stat["channel_id"].as_str().unwrap_or("").to_string(),
                index: stat["index"].as_i64().unwrap_or(0) as i32,
                account_id: stat["account_id"].as_str().unwrap_or("").to_string(),
                balance_xlm: sqlx::types::Decimal::from_f64_retain(balance).unwrap_or_default(),
                current_sequence: stat["current_sequence"].as_i64().unwrap_or(0),
                reserved_sequence: stat["reserved_sequence"].as_i64().unwrap_or(0),
                in_flight_transactions: stat["in_flight"].as_i64().unwrap_or(0),
                total_submitted: stat["total_submitted"].as_u64().unwrap_or(0),
                total_successful: stat["total_successful"].as_u64().unwrap_or(0),
                total_failed: stat["total_failed"].as_u64().unwrap_or(0),
                consecutive_failures: stat["consecutive_failures"].as_u64().unwrap_or(0) as u32,
                is_circuit_broken: stat["is_circuit_broken"].as_bool().unwrap_or(false),
                status,
            }
        })
        .collect();

    Ok(Json(AdminResponse {
        success: true,
        data: Some(channels),
        error: None,
    }))
}

/// Queue a top-up for a channel account
async fn queue_channel_topup(
    State(state): State<StellarAdminState>,
    Path(channel_index): Path<i32>,
    Json(payload): Json<ChannelTopUpRequest>,
) -> Result<Json<AdminResponse<TopUpQueueResponse>>, AdminError> {
    // Validate channel exists
    let _channel = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM stellar_submission_channels WHERE channel_index = $1",
    )
    .bind(channel_index)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AdminError::NotFound(format!("Channel {} not found", channel_index)))?;

    // Queue the top-up operation
    let operation_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO stellar_channel_topup_queue (
            id, channel_index, amount_xlm, description,
            status, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'pending', NOW(), NOW())
        "#,
    )
    .bind(operation_id)
    .bind(channel_index)
    .bind(payload.amount_xlm)
    .bind(payload.description.unwrap_or_default())
    .execute(&state.pool)
    .await?;

    Ok(Json(AdminResponse {
        success: true,
        data: Some(TopUpQueueResponse {
            operation_id: operation_id.to_string(),
            channel_index,
            amount_xlm: payload.amount_xlm,
            status: "queued".to_string(),
        }),
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
pub struct QueueTickQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct QueueTickResponse {
    pub processed: usize,
}

#[derive(Debug, Serialize)]
pub struct TopUpQueueResponse {
    pub operation_id: String,
    pub channel_index: i32,
    pub amount_xlm: sqlx::types::Decimal,
    pub status: String,
}

/// Admin error type
#[derive(Debug)]
pub enum AdminError {
    NotFound(String),
    BadRequest(String),
    InternalError(String),
    Unauthorized,
    TooManyRequests(String),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AdminError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AdminError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AdminError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AdminError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AdminError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
        };

        (
            status,
            Json(AdminResponse::<()> {
                success: false,
                data: None,
                error: Some(message),
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AdminError {
    fn from(err: sqlx::Error) -> Self {
        AdminError::InternalError(err.to_string())
    }
}

impl From<crate::stellar::error::SubmissionError> for AdminError {
    fn from(err: crate::stellar::error::SubmissionError) -> Self {
        match err {
            crate::stellar::error::SubmissionError::QueueTimeout { .. } => {
                AdminError::TooManyRequests(err.to_string())
            }
            other => AdminError::InternalError(other.to_string()),
        }
    }
}

async fn enqueue_batch_submissions(
    State(state): State<StellarAdminState>,
    Json(payload): Json<Vec<BatchEnvelopeRequest>>,
) -> Result<Json<AdminResponse<BatchSubmissionResult>>, AdminError> {
    let result = state.submission_engine.enqueue_batch(payload).await?;
    Ok(Json(AdminResponse {
        success: true,
        data: Some(result),
        error: None,
    }))
}

async fn process_queue_tick(
    State(state): State<StellarAdminState>,
    Query(q): Query<QueueTickQuery>,
) -> Result<Json<AdminResponse<QueueTickResponse>>, AdminError> {
    let processed = state
        .submission_engine
        .process_submission_queue_tick(q.limit.unwrap_or(100))
        .await?;

    Ok(Json(AdminResponse {
        success: true,
        data: Some(QueueTickResponse { processed }),
        error: None,
    }))
}

/// Get a snapshot of submission engine metrics
async fn get_submission_metrics(
    State(state): State<StellarAdminState>,
) -> Result<Json<AdminResponse<SubmissionMetrics>>, AdminError> {
    let metrics = state.submission_engine.get_metrics_snapshot().await?;
    Ok(Json(AdminResponse {
        success: true,
        data: Some(metrics),
        error: None,
    }))
}

/// Query forensic failure records from `stellar_tx_forensic_failures`
async fn get_forensic_failures(
    State(state): State<StellarAdminState>,
    Query(q): Query<ForensicsQuery>,
) -> Result<Json<AdminResponse<Vec<ForensicFailureRow>>>, AdminError> {
    let limit = q.limit.unwrap_or(100);

    let rows = if let Some(error_code) = q.error_code {
        sqlx::query_as::<_, ForensicFailureRow>(
            r#"
            SELECT id, queue_id, tx_log_id, issuer_id, channel_id,
                   error_code, error_reason, horizon_status, retryable,
                   occurred_at, created_at
            FROM stellar_tx_forensic_failures
            WHERE error_code = $1
            ORDER BY occurred_at DESC
            LIMIT $2
            "#,
        )
        .bind(error_code)
        .bind(limit)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, ForensicFailureRow>(
            r#"
            SELECT id, queue_id, tx_log_id, issuer_id, channel_id,
                   error_code, error_reason, horizon_status, retryable,
                   occurred_at, created_at
            FROM stellar_tx_forensic_failures
            ORDER BY occurred_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&state.pool)
        .await?
    };

    Ok(Json(AdminResponse {
        success: true,
        data: Some(rows),
        error: None,
    }))
}

/// Create admin routes for Stellar management
pub fn stellar_admin_routes(state: StellarAdminState) -> Router {
    Router::new()
        .route("/api/admin/stellar/channels", get(get_channels))
        .route(
            "/api/admin/stellar/channels/:index/top-up",
            post(queue_channel_topup),
        )
        .route(
            "/api/admin/stellar/submission/batch",
            post(enqueue_batch_submissions),
        )
        .route(
            "/api/admin/stellar/submission/queue/process",
            post(process_queue_tick),
        )
        .route("/api/admin/stellar/metrics", get(get_submission_metrics))
        .route("/api/admin/stellar/forensics", get(get_forensic_failures))
        .with_state(state)
}

use sqlx::types::Decimal;
