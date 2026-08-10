mod api;
mod auth;
pub mod blockchain;
mod config;
mod error;
mod models;
mod payments;
mod services;

pub use config::AppConfig;

use sqlx::{postgres::PgPoolOptions, PgPool};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: std::sync::Arc<String>,
    pub webhook_secret: std::sync::Arc<String>,
}

pub async fn build_state(config: &AppConfig) -> Result<AppState, sqlx::Error> {
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    Ok(AppState {
        db,
        jwt_secret: config.jwt_secret.clone(),
        webhook_secret: config.webhook_secret.clone(),
    })
}

pub fn router(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(|| async { "aframp" }))
        .route(
            "/health",
            axum::routing::get(|| async { axum::http::StatusCode::NO_CONTENT }),
        )
        .route("/signup", axum::routing::post(api::auth::signup))
        .route("/login", axum::routing::post(api::auth::login))
        .route("/wallet/create", axum::routing::post(api::wallets::create))
        .route("/wallet", axum::routing::get(api::wallets::get))
        .route("/balance", axum::routing::get(api::balances::get))
        .route("/transactions", axum::routing::get(api::transactions::list))
        .route("/withdraw", axum::routing::post(api::withdrawals::create))
        .route("/withdrawals", axum::routing::get(api::withdrawals::list))
        .with_state(state)
}
