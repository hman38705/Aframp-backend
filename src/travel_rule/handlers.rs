use crate::middleware::rbac::{CallerIdentity, ROLE_COMPLIANCE_OFFICER, ROLE_FINANCE_DIRECTOR};
use crate::travel_rule::models::*;
use crate::travel_rule::repository::TravelRuleRepository;
use crate::travel_rule::service::TravelRuleService;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TravelRuleState {
    pub repo: Arc<TravelRuleRepository>,
    pub service: Arc<TravelRuleService>,
}

// ---------------------------------------------------------------------------
// Generic response helpers
// ---------------------------------------------------------------------------

fn ok<T: Serialize>(data: T) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "success": true, "data": data })))
}

fn created<T: Serialize>(data: T) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::CREATED, Json(json!({ "success": true, "data": data })))
}

fn err_400(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": msg })))
}

fn err_403(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::FORBIDDEN, Json(json!({ "success": false, "error": msg })))
}

fn err_404(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": msg })))
}

fn err_500(e: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!(error = %e, "Travel Rule internal error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "success": false, "error": e.to_string() })),
    )
}

fn require_compliance_officer(caller: &CallerIdentity) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if caller.role != ROLE_COMPLIANCE_OFFICER && caller.role != ROLE_FINANCE_DIRECTOR {
        return Err(err_403("compliance_officer or finance_director role required"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// VASP Registry handlers
// ---------------------------------------------------------------------------

/// POST /api/admin/compliance/travel-rule/vasps
pub async fn register_vasp(
    State(state): State<Arc<TravelRuleState>>,
    Extension(caller): Extension<CallerIdentity>,
    Json(req): Json<RegisterVaspRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_compliance_officer(&caller) {
        return e.into_response();
    }
    match state.repo.create_vasp(&req).await {
        Ok(v) => created(v).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// GET /api/admin/compliance/travel-rule/vasps
pub async fn list_vasps(
    State(state): State<Arc<TravelRuleState>>,
    Query(query): Query<ListVaspsQuery>,
) -> impl IntoResponse {
    match state.repo.list_vasps(&query).await {
        Ok(vasps) => ok(vasps).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// GET /api/admin/compliance/travel-rule/vasps/:vasp_id
pub async fn get_vasp(
    State(state): State<Arc<TravelRuleState>>,
    Path(vasp_id): Path<String>,
) -> impl IntoResponse {
    match state.repo.get_vasp(&vasp_id).await {
        Ok(Some(v)) => ok(v).into_response(),
        Ok(None) => err_404("VASP not found").into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// PATCH /api/admin/compliance/travel-rule/vasps/:vasp_id
pub async fn update_vasp(
    State(state): State<Arc<TravelRuleState>>,
    Extension(caller): Extension<CallerIdentity>,
    Path(vasp_id): Path<String>,
    Json(req): Json<UpdateVaspRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_compliance_officer(&caller) {
        return e.into_response();
    }
    match state.repo.update_vasp(&vasp_id, &req).await {
        Ok(v) => ok(v).into_response(),
        Err(e) if e.to_string().contains("not found") => err_404("VASP not found").into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Threshold handlers
// ---------------------------------------------------------------------------

/// GET /api/admin/compliance/travel-rule/thresholds
pub async fn get_thresholds(
    State(state): State<Arc<TravelRuleState>>,
) -> impl IntoResponse {
    match state.repo.list_thresholds().await {
        Ok(t) => ok(t).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// PATCH /api/admin/compliance/travel-rule/thresholds
/// Requires compliance_officer role.
pub async fn update_thresholds(
    State(state): State<Arc<TravelRuleState>>,
    Extension(caller): Extension<CallerIdentity>,
    Json(req): Json<UpdateThresholdsRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_compliance_officer(&caller) {
        return e.into_response();
    }
    let mut updated = Vec::new();
    for item in &req.thresholds {
        match state.repo.upsert_threshold(item, req.approved_by).await {
            Ok(t) => updated.push(t),
            Err(e) => return err_500(e).into_response(),
        }
    }
    ok(updated).into_response()
}

// ---------------------------------------------------------------------------
// Unhosted wallet policy handlers
// ---------------------------------------------------------------------------

/// GET /api/admin/compliance/travel-rule/unhosted-wallet-policy
pub async fn get_unhosted_policy(
    State(state): State<Arc<TravelRuleState>>,
) -> impl IntoResponse {
    match state.repo.get_active_policy().await {
        Ok(Some(p)) => ok(p).into_response(),
        Ok(None) => err_404("No unhosted wallet policy configured").into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// PATCH /api/admin/compliance/travel-rule/unhosted-wallet-policy
/// Requires compliance_officer role AND a distinct senior_management_id (dual approval).
pub async fn update_unhosted_policy(
    State(state): State<Arc<TravelRuleState>>,
    Extension(caller): Extension<CallerIdentity>,
    Json(req): Json<UpdateUnhostedWalletPolicyRequest>,
) -> impl IntoResponse {
    if caller.role != ROLE_COMPLIANCE_OFFICER {
        return err_403("compliance_officer role required for unhosted wallet policy changes")
            .into_response();
    }
    // Dual approval: compliance officer and senior management must be different users
    if req.compliance_officer_id == req.senior_management_id {
        return err_400(
            "compliance_officer_id and senior_management_id must be different users"
        ).into_response();
    }
    const VALID_POLICIES: &[&str] =
        &["allow", "allow_below_threshold", "require_attestation", "block"];
    if !VALID_POLICIES.contains(&req.policy_type.as_str()) {
        return err_400(&format!(
            "policy_type must be one of: {}",
            VALID_POLICIES.join(", ")
        ))
        .into_response();
    }
    match state.repo.update_policy(&req).await {
        Ok(p) => ok(p).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Message management handlers
// ---------------------------------------------------------------------------

/// GET /api/admin/compliance/travel-rule/messages
pub async fn list_messages(
    State(state): State<Arc<TravelRuleState>>,
    Query(query): Query<TravelRuleMessageListQuery>,
) -> impl IntoResponse {
    match state.repo.list_exchanges(&query).await {
        Ok(msgs) => ok(msgs).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// GET /api/admin/compliance/travel-rule/messages/:message_id
pub async fn get_message(
    State(state): State<Arc<TravelRuleState>>,
    Path(message_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.repo.get_exchange(message_id).await {
        Ok(Some(msg)) => ok(msg).into_response(),
        Ok(None) => err_404("Message not found").into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

/// POST /api/admin/compliance/travel-rule/messages/:message_id/retry
pub async fn retry_message(
    State(state): State<Arc<TravelRuleState>>,
    Extension(caller): Extension<CallerIdentity>,
    Path(message_id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = require_compliance_officer(&caller) {
        return e.into_response();
    }
    match state.service.retry_failed_message(message_id).await {
        Ok(msg) => ok(msg).into_response(),
        Err(e) if e.to_string().contains("not in a retryable state") => {
            err_400(&e.to_string()).into_response()
        }
        Err(e) if e.to_string().contains("not found") => err_404("Message not found").into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Inbound Travel Rule receive endpoint (public — called by counterpart VASPs)
// ---------------------------------------------------------------------------

/// POST /api/compliance/travel-rule/inbound/:protocol
pub async fn receive_inbound(
    State(state): State<Arc<TravelRuleState>>,
    Path(protocol): Path<String>,
    Json(data): Json<InboundTravelRuleData>,
) -> impl IntoResponse {
    match state.service.handle_inbound(data).await {
        Ok(exchange) => ok(json!({
            "exchange_id": exchange.exchange_id,
            "status": exchange.status,
            "acknowledged": true,
        }))
        .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, protocol = %protocol, "Inbound Travel Rule handling failed");
            err_400(&e.to_string()).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Self-attestation handler
// ---------------------------------------------------------------------------

/// POST /api/compliance/travel-rule/attest
pub async fn submit_attestation(
    State(state): State<Arc<TravelRuleState>>,
    Json(req): Json<SelfAttestationRequest>,
) -> impl IntoResponse {
    match state.service.submit_attestation(req).await {
        Ok(att) => created(att).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Metrics / reporting handler
// ---------------------------------------------------------------------------

/// GET /api/admin/compliance/travel-rule/metrics
pub async fn get_metrics(
    State(state): State<Arc<TravelRuleState>>,
    Query(query): Query<TravelRuleMetricsQuery>,
) -> impl IntoResponse {
    match state.repo.compute_metrics(query.from, query.to).await {
        Ok(m) => ok(m).into_response(),
        Err(e) => err_500(e).into_response(),
    }
}
