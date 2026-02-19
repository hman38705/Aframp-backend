// Example usage of the logging and error handling system
// This file demonstrates how to use the new structured logging and error handling

// Note: Replace Bitmesh_backend with your actual crate name
#[cfg(feature = "database")]
use Bitmesh_backend as aframp;

#[cfg(feature = "database")]
use aframp::{
    error::{AppError, AppErrorKind, ValidationError},
    logging::{init_tracing, mask_wallet_address},
    middleware::{
        error::success_response,
        logging::{
            log_database_query, log_external_call, request_logging_middleware, UuidRequestId,
        },
    },
};

#[cfg(feature = "database")]
use axum::{
    routing::{get, post},
    Json, Router,
};
#[cfg(feature = "database")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "database")]
use tower::ServiceBuilder;
#[cfg(feature = "database")]
use tower_http::request_id::{PropagateRequestIdLayer, SetRequestIdLayer};
#[cfg(feature = "database")]
use tracing::info;

#[cfg(feature = "database")]
#[derive(Debug, Deserialize)]
struct OnrampRequest {
    wallet_address: String,
    cngn_amount: f64,
    currency: String,
}

#[cfg(feature = "database")]
#[derive(Debug, Serialize)]
struct OnrampResponse {
    transaction_id: String,
    status: String,
}

/// Example handler showing error handling
#[cfg(feature = "database")]
async fn onramp_handler(
    Json(request): Json<OnrampRequest>,
) -> Result<Json<OnrampResponse>, AppError> {
    // Log the incoming request with masked wallet
    info!(
        wallet = %mask_wallet_address(&request.wallet_address),
        amount = %request.cngn_amount,
        currency = %request.currency,
        "Onramp request received"
    );

    // Validate wallet address
    if request.wallet_address.len() != 56 {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: request.wallet_address,
                reason: "Must be 56 characters".to_string(),
            },
        )));
    }

    // Validate amount
    if request.cngn_amount <= 0.0 {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidAmount {
                amount: request.cngn_amount.to_string(),
                reason: "Amount must be positive".to_string(),
            },
        )));
    }

    // Simulate database query with logging
    let transaction_id = log_database_query("INSERT INTO transactions VALUES (...)", async {
        // Your database operation here
        Ok::<_, AppError>("tx_12345".to_string())
    })
    .await?;

    // Simulate external API call with logging
    log_external_call("Stellar", "POST /transactions", async {
        // Your external API call here
        Ok::<_, AppError>(())
    })
    .await?;

    // Log successful completion
    info!(
        transaction_id = %transaction_id,
        wallet = %mask_wallet_address(&request.wallet_address),
        "Onramp completed successfully"
    );

    Ok(Json(OnrampResponse {
        transaction_id,
        status: "completed".to_string(),
    }))
}

/// Example handler showing success responses
#[cfg(feature = "database")]
async fn health_check() -> impl axum::response::IntoResponse {
    success_response(serde_json::json!({
        "status": "healthy",
        "version": "1.0.0"
    }))
}

/// Build the application with all middleware
#[cfg(feature = "database")]
pub fn build_app() -> Router {
    // Initialize tracing (call once at app startup)
    init_tracing();

    // Build the router with middleware
    Router::new()
        .route("/health", get(health_check))
        .route("/api/onramp", post(onramp_handler))
        .layer(
            ServiceBuilder::new()
                // Generate request IDs
                .layer(SetRequestIdLayer::x_request_id(UuidRequestId))
                // Log requests and responses
                .layer(axum::middleware::from_fn(request_logging_middleware))
                // Propagate request IDs in response headers
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
}

#[cfg(feature = "database")]
#[tokio::main]
async fn main() {
    let app = build_app();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}

#[cfg(not(feature = "database"))]
fn main() {
    println!("This example requires the 'database' feature to be enabled.");
    println!("Run with: cargo run --features database --example usage");
}
