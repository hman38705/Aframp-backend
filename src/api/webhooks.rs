use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::services::webhook_processor::{WebhookProcessor, WebhookProcessorError};

pub struct WebhookState {
    pub processor: Arc<WebhookProcessor>,
}

/// POST /webhooks/:provider
pub async fn handle_webhook(
    State(state): State<Arc<WebhookState>>,
    Path(provider): Path<String>,
    headers: axum::http::HeaderMap,
    body: String,
) -> impl IntoResponse {
    info!(provider = %provider, "Received webhook");

    // Extract signature from headers
    let signature = match provider.as_str() {
        "flutterwave" => headers
            .get("verif-hash")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
        "paystack" => headers
            .get("x-paystack-signature")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string()),
        _ => None,
    };

    if signature.is_none() {
        warn!(provider = %provider, "Missing webhook signature");
        return (StatusCode::UNAUTHORIZED, "Missing signature").into_response();
    }

    // Parse payload
    let payload: JsonValue = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            error!(provider = %provider, error = %e, "Invalid JSON payload");
            return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response();
        }
    };

    // Process webhook
    match state
        .processor
        .process_webhook(&provider, signature.as_deref(), &payload)
        .await
    {
        Ok(_) => {
            info!(provider = %provider, "Webhook processed successfully");
            (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
        }
        Err(WebhookProcessorError::InvalidSignature) => {
            warn!(provider = %provider, "Invalid webhook signature");
            (StatusCode::UNAUTHORIZED, "Invalid signature").into_response()
        }
        Err(WebhookProcessorError::AlreadyProcessed) => {
            info!(provider = %provider, "Webhook already processed");
            (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
        }
        Err(e) => {
            error!(provider = %provider, error = %e, "Webhook processing failed");
            (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
        }
    }
}
