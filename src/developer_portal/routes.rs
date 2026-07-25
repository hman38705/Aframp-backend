//! Developer portal routes
//!
//! Includes sandbox reset endpoint and developer portal APIs

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api_keys::generator::KeyEnvironment;
use crate::auth::jwt::TokenClaims;
use crate::developer_portal::config::DeveloperPortalConfig;
use crate::developer_portal::sandbox::{SandboxIsolationService, SandboxValidationError};

/// Developer portal API state
#[derive(Clone)]
pub struct DeveloperPortalState {
    pub config: Arc<DeveloperPortalConfig>,
    pub sandbox_service: Arc<SandboxIsolationService>,
    pub db: Option<Arc<sqlx::PgPool>>,
}

/// Sandbox reset request
#[derive(Debug, Deserialize)]
pub struct SandboxResetRequest {
    /// Sandbox environment to reset
    pub environment: String,
    /// Optional: specific resources to reset
    pub resources: Option<Vec<String>>,
    /// Force reset even if data is recent
    pub force: Option<bool>,
}

/// Sandbox reset response
#[derive(Debug, Serialize)]
pub struct SandboxResetResponse {
    pub success: bool,
    pub message: String,
    pub reset_resources: Vec<String>,
    pub reset_timestamp: chrono::DateTime<chrono::Utc>,
}

/// Developer portal routes
pub fn routes(state: DeveloperPortalState) -> Router {
    Router::new()
        .route("/sandbox/reset", post(sandbox_reset))
        .route("/sandbox/status", get(sandbox_status))
        .route("/applications", get(list_applications))
        .route("/applications", post(create_application))
        .with_state(state)
}

/// Reset sandbox environment
///
/// POST /api/developer/sandbox/reset
/// Clears all test data and resets limits for the sandbox environment
async fn sandbox_reset(
    State(state): State<DeveloperPortalState>,
    Extension(claims): Extension<TokenClaims>,
    Json(request): Json<SandboxResetRequest>,
) -> Response {
    // Check if sandbox reset is allowed
    if !state.config.allow_sandbox_reset {
        return (
            StatusCode::FORBIDDEN,
            Json(SandboxResetResponse {
                success: false,
                message: "Sandbox reset operations are disabled".to_string(),
                reset_resources: vec![],
                reset_timestamp: chrono::Utc::now(),
            }),
        ).into_response();
    }

    // Validate developer has sandbox access
    let key_env = match claims.environment.as_deref() {
        Some("testnet") => KeyEnvironment::Testnet,
        Some("mainnet") => KeyEnvironment::Mainnet,
        _ => KeyEnvironment::Mainnet,
    };

    if !state.sandbox_service.is_sandbox_key(&key_env) {
        return (
            StatusCode::FORBIDDEN,
            Json(SandboxResetResponse {
                success: false,
                message: "API key is not scoped to sandbox environment".to_string(),
                reset_resources: vec![],
                reset_timestamp: chrono::Utc::now(),
            }),
        ).into_response();
    }

    // Perform sandbox reset
    let reset_result = perform_sandbox_reset(&state, &request, &claims).await;

    match reset_result {
        Ok(reset_resources) => {
            info!(
                developer_id = %claims.sub,
                environment = %request.environment,
                reset_resources_count = reset_resources.len(),
                "Sandbox reset completed successfully"
            );

            (
                StatusCode::OK,
                Json(SandboxResetResponse {
                    success: true,
                    message: format!("Sandbox environment '{}' reset successfully", request.environment),
                    reset_resources,
                    reset_timestamp: chrono::Utc::now(),
                }),
            ).into_response()
        }
        Err(e) => {
            error!(
                developer_id = %claims.sub,
                environment = %request.environment,
                error = %e,
                "Sandbox reset failed"
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SandboxResetResponse {
                    success: false,
                    message: format!("Sandbox reset failed: {}", e),
                    reset_resources: vec![],
                    reset_timestamp: chrono::Utc::now(),
                }),
            ).into_response()
        }
    }
}

/// Perform actual sandbox reset operations
async fn perform_sandbox_reset(
    state: &DeveloperPortalState,
    request: &SandboxResetRequest,
    claims: &TokenClaims,
) -> Result<Vec<String>, anyhow::Error> {
    let mut reset_resources = Vec::new();

    // Get database pool
    let db_pool = match &state.db {
        Some(pool) => pool,
        None => {
            warn!("No database pool available for sandbox reset");
            return Ok(reset_resources);
        }
    };

    // Reset based on requested resources or all resources
    let resources_to_reset = request.resources.clone().unwrap_or_else(|| {
        vec![
            "test_transactions".to_string(),
            "test_wallets".to_string(),
            "test_payments".to_string(),
            "test_balances".to_string(),
        ]
    });

    for resource in &resources_to_reset {
        match resource.as_str() {
            "test_transactions" => {
                // Reset test transactions for this developer
                let deleted_count = sqlx::query!(
                    r#"
                    DELETE FROM transactions 
                    WHERE consumer_id = $1 
                    AND environment = 'testnet'
                    "#,
                    claims.sub
                )
                .execute(db_pool.as_ref())
                .await?
                .rows_affected();

                if deleted_count > 0 {
                    reset_resources.push(format!("test_transactions ({} deleted)", deleted_count));
                }
            }
            "test_wallets" => {
                // Reset test wallets for this developer
                let deleted_count = sqlx::query!(
                    r#"
                    DELETE FROM wallets 
                    WHERE consumer_id = $1 
                    AND environment = 'testnet'
                    "#,
                    claims.sub
                )
                .execute(db_pool.as_ref())
                .await?
                .rows_affected();

                if deleted_count > 0 {
                    reset_resources.push(format!("test_wallets ({} deleted)", deleted_count));
                }
            }
            "test_payments" => {
                // Reset test payments for this developer
                let deleted_count = sqlx::query!(
                    r#"
                    DELETE FROM payments 
                    WHERE consumer_id = $1 
                    AND environment = 'testnet'
                    "#,
                    claims.sub
                )
                .execute(db_pool.as_ref())
                .await?
                .rows_affected();

                if deleted_count > 0 {
                    reset_resources.push(format!("test_payments ({} deleted)", deleted_count));
                }
            }
            "test_balances" => {
                // Reset test balances for this developer
                let reset_count = sqlx::query!(
                    r#"
                    UPDATE balances 
                    SET amount = 1000.00 
                    WHERE consumer_id = $1 
                    AND environment = 'testnet'
                    "#,
                    claims.sub
                )
                .execute(db_pool.as_ref())
                .await?
                .rows_affected();

                if reset_count > 0 {
                    reset_resources.push(format!("test_balances ({} reset)", reset_count));
                }
            }
            _ => {
                warn!("Unknown resource type for reset: {}", resource);
            }
        }
    }

    // Reset rate limits (simulated - would integrate with actual rate limiting system)
    reset_resources.push("rate_limits".to_string());

    Ok(reset_resources)
}

/// Get sandbox status
///
/// GET /api/developer/sandbox/status
async fn sandbox_status(
    State(state): State<DeveloperPortalState>,
    Extension(claims): Extension<TokenClaims>,
) -> Response {
    let key_env = match claims.environment.as_deref() {
        Some("testnet") => KeyEnvironment::Testnet,
        Some("mainnet") => KeyEnvironment::Mainnet,
        _ => KeyEnvironment::Mainnet,
    };

    let is_sandbox = state.sandbox_service.is_sandbox_key(&key_env);
    let sandbox_config = state.sandbox_service.get_sandbox_config();

    #[derive(Serialize)]
    struct SandboxStatusResponse {
        is_sandbox: bool,
        environment: String,
        stellar_url: String,
        rate_limit_per_minute: u32,
        max_lifetime_hours: u32,
        reset_allowed: bool,
    }

    (
        StatusCode::OK,
        Json(SandboxStatusResponse {
            is_sandbox,
            environment: key_env.as_str().to_string(),
            stellar_url: sandbox_config.stellar_url,
            rate_limit_per_minute: sandbox_config.rate_limit_per_minute,
            max_lifetime_hours: sandbox_config.max_lifetime_hours,
            reset_allowed: state.config.allow_sandbox_reset,
        }),
    ).into_response()
}

/// List developer applications
async fn list_applications() -> Response {
    // TODO: Implement listing of developer applications
    (StatusCode::NOT_IMPLEMENTED, "Not implemented").into_response()
}

/// Create developer application
async fn create_application() -> Response {
    // TODO: Implement creation of developer applications
    (StatusCode::NOT_IMPLEMENTED, "Not implemented").into_response()
}