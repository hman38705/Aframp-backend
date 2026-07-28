use crate::admin::mint_signer_models::*;
use crate::admin::mint_signer_service::MintSignerService;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        success: true,
        data,
    })
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// POST /api/admin/mint/signers
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers",
    tag = "admin",
    summary = "Initiate mint signer onboarding",
    description = "Starts the onboarding process for a new mint quorum signer and returns a one-time onboarding token.",
    request_body = InitiateOnboardingRequest,
    responses(
        (status = 201, description = "Onboarding initiated", body = serde_json::Value),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn initiate_onboarding(
    State(svc): State<Arc<MintSignerService>>,
    Json(req): Json<InitiateOnboardingRequest>,
) -> Result<(StatusCode, Json<ApiResponse<serde_json::Value>>), (StatusCode, String)> {
    // In production, initiated_by comes from the auth context extension
    let initiated_by = Uuid::nil(); // placeholder — wire from AdminAuthContext
    let (signer, token) = svc
        .initiate_onboarding(req, initiated_by)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({ "signer_id": signer.id, "onboarding_token": token }),
        }),
    ))
}

/// POST /api/admin/mint/signers/complete-onboarding
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/complete-onboarding",
    tag = "admin",
    summary = "Complete mint signer onboarding",
    description = "Completes onboarding by registering the signer's Stellar public key using the onboarding token and challenge signature.",
    request_body = CompleteOnboardingRequest,
    responses(
        (status = 200, description = "Onboarding completed", body = MintSigner),
        (status = 400, description = "Invalid request or signature"),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn complete_onboarding(
    State(svc): State<Arc<MintSignerService>>,
    Json(req): Json<CompleteOnboardingRequest>,
) -> Result<Json<ApiResponse<MintSigner>>, (StatusCode, String)> {
    let signer = svc
        .complete_onboarding(req, None)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(signer))
}

/// POST /api/admin/mint/signers/:id/confirm-identity
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/{id}/confirm-identity",
    tag = "admin",
    summary = "Confirm mint signer identity",
    description = "Marks a mint signer's identity as verified.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    responses(
        (status = 200, description = "Identity confirmed"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn confirm_identity(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    svc.confirm_identity(id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(()))
}

/// POST /api/admin/mint/signers/:id/challenge
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/{id}/challenge",
    tag = "admin",
    summary = "Request a signing challenge",
    description = "Generates a signing challenge for the signer to prove control of their private key.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    responses(
        (status = 200, description = "Challenge generated", body = serde_json::Value),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn request_challenge(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    let challenge = svc
        .generate_challenge(id, None)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(serde_json::json!({ "challenge": challenge })))
}

/// POST /api/admin/mint/signers/:id/rotate-key
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/{id}/rotate-key",
    tag = "admin",
    summary = "Initiate signer key rotation",
    description = "Initiates rotation of a mint signer's Stellar key, starting the grace period for the old key.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    request_body = RotateKeyRequest,
    responses(
        (status = 200, description = "Key rotation initiated", body = MintSignerKeyRotation),
        (status = 400, description = "Invalid request or signature"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn rotate_key(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RotateKeyRequest>,
) -> Result<Json<ApiResponse<MintSignerKeyRotation>>, (StatusCode, String)> {
    let initiated_by = Uuid::nil(); // wire from auth context
    let rotation = svc
        .initiate_key_rotation(id, req, initiated_by, None)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(rotation))
}

/// POST /api/admin/mint/signers/:id/rotate-key/challenge
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/{id}/rotate-key/challenge",
    tag = "admin",
    summary = "Request a key rotation challenge",
    description = "Generates a signing challenge for a pending key rotation, binding it to the new Stellar public key.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Rotation challenge generated", body = serde_json::Value),
        (status = 400, description = "Missing or invalid new_stellar_public_key"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn request_rotation_challenge(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    let new_key = body
        .get("new_stellar_public_key")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::BAD_REQUEST,
            "new_stellar_public_key required".into(),
        ))?;
    let challenge = svc
        .generate_rotation_challenge(id, new_key, None)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(serde_json::json!({ "challenge": challenge })))
}

/// POST /api/admin/mint/signers/:id/suspend
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/{id}/suspend",
    tag = "admin",
    summary = "Suspend a mint signer",
    description = "Suspends a mint signer, preventing them from participating in mint approvals.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    request_body = SuspendSignerRequest,
    responses(
        (status = 200, description = "Signer suspended"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn suspend_signer(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SuspendSignerRequest>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    svc.suspend(id, &req.reason)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(()))
}

/// POST /api/admin/mint/signers/:id/remove
#[utoipa::path(
    post,
    path = "/api/admin/mint/signers/{id}/remove",
    tag = "admin",
    summary = "Remove a mint signer",
    description = "Permanently removes a mint signer from the quorum.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    responses(
        (status = 200, description = "Signer removed"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove_signer(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    svc.remove(id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(()))
}

/// GET /api/admin/mint/signers
#[utoipa::path(
    get,
    path = "/api/admin/mint/signers",
    tag = "admin",
    summary = "List mint signers",
    description = "Lists all mint quorum signers with a summary of their status and signing activity.",
    responses(
        (status = 200, description = "List of signers", body = Vec<SignerSummary>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_signers(
    State(svc): State<Arc<MintSignerService>>,
) -> Result<Json<ApiResponse<Vec<SignerSummary>>>, (StatusCode, String)> {
    let signers = svc
        .repo_list_all()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let summaries = signers
        .into_iter()
        .map(|s| SignerSummary {
            id: s.id,
            full_legal_name: s.full_legal_name,
            role: s.role,
            organisation: s.organisation,
            status: s.status,
            key_fingerprint: s.key_fingerprint,
            last_signing_at: s.last_signing_at,
            key_expires_at: s.key_expires_at,
        })
        .collect();
    Ok(ok(summaries))
}

/// GET /api/admin/mint/signers/:id
#[utoipa::path(
    get,
    path = "/api/admin/mint/signers/{id}",
    tag = "admin",
    summary = "Get a mint signer",
    description = "Retrieves full details for a single mint signer.",
    params(
        ("id" = Uuid, Path, description = "Signer ID")
    ),
    responses(
        (status = 200, description = "Signer details", body = MintSigner),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_signer(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<MintSigner>>, (StatusCode, String)> {
    let signer = svc
        .repo_find_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or((StatusCode::NOT_FOUND, "Signer not found".into()))?;
    Ok(ok(signer))
}

/// GET /api/admin/mint/signers/:id/activity
#[utoipa::path(
    get,
    path = "/api/admin/mint/signers/{id}/activity",
    tag = "admin",
    summary = "Get mint signer signing activity",
    description = "Retrieves a paginated list of signing activity for a mint signer.",
    params(
        ("id" = Uuid, Path, description = "Signer ID"),
        ("limit" = Option<i64>, Query, description = "Maximum number of records to return"),
        ("offset" = Option<i64>, Query, description = "Number of records to skip")
    ),
    responses(
        (status = 200, description = "Signer activity retrieved", body = Vec<MintSignerActivity>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Signer not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_signer_activity(
    State(svc): State<Arc<MintSignerService>>,
    Path(id): Path<Uuid>,
    Query(q): Query<PaginationQuery>,
) -> Result<Json<ApiResponse<Vec<MintSignerActivity>>>, (StatusCode, String)> {
    let activity = svc
        .repo_list_activity(id, q.limit.unwrap_or(50), q.offset.unwrap_or(0))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(ok(activity))
}

/// GET /api/admin/mint/quorum
#[utoipa::path(
    get,
    path = "/api/admin/mint/quorum",
    tag = "admin",
    summary = "Get mint quorum status",
    description = "Retrieves the current mint quorum configuration and whether quorum is reachable.",
    responses(
        (status = 200, description = "Quorum status retrieved", body = QuorumStatus),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_quorum(
    State(svc): State<Arc<MintSignerService>>,
) -> Result<Json<ApiResponse<QuorumStatus>>, (StatusCode, String)> {
    let status = svc
        .get_quorum_status()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(ok(status))
}

/// PATCH /api/admin/mint/quorum
#[utoipa::path(
    patch,
    path = "/api/admin/mint/quorum",
    tag = "admin",
    summary = "Update mint quorum configuration",
    description = "Updates the required approval threshold and/or minimum role diversity for the mint quorum.",
    request_body = UpdateQuorumRequest,
    responses(
        (status = 200, description = "Quorum configuration updated", body = MintQuorumConfig),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_quorum(
    State(svc): State<Arc<MintSignerService>>,
    Json(req): Json<UpdateQuorumRequest>,
) -> Result<Json<ApiResponse<MintQuorumConfig>>, (StatusCode, String)> {
    let updated_by = Uuid::nil(); // wire from auth context
    let cfg = svc
        .update_quorum(req, updated_by)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(ok(cfg))
}
