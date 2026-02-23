//! Integration tests for the rates API endpoint
//!
//! Tests cover:
//! - Single pair queries
//! - Multiple pairs queries
//! - All pairs queries
//! - Caching behavior
//! - Error handling
//! - CORS headers
//! - ETag support

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use aframp_backend::api::rates::{get_rates, options_rates, RatesState};
use aframp_backend::cache::cache::RedisCache;
use aframp_backend::database::connection::create_pool;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use aframp_backend::services::exchange_rate::{ExchangeRateService, ExchangeRateServiceConfig};
use std::sync::Arc;

async fn create_test_app() -> Router {
    // Use test database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/aframp_test".to_string());
    
    let db_pool = create_pool(&database_url)
        .await
        .expect("Failed to create test database pool");

    // Create exchange rate service
    let repository = ExchangeRateRepository::new(db_pool.clone());
    let config = ExchangeRateServiceConfig::default();
    let exchange_rate_service = ExchangeRateService::new(repository, config);

    let rates_state = RatesState {
        exchange_rate_service: Arc::new(exchange_rate_service),
        cache: None, // No cache for tests to ensure fresh data
    };

    Router::new()
        .route("/api/rates", axum::routing::get(get_rates).options(options_rates))
        .with_state(rates_state)
}

#[tokio::test]
async fn test_single_pair_ngn_to_cngn() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN&to=cNGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["pair"], "NGN/cNGN");
    assert_eq!(json["base_currency"], "NGN");
    assert_eq!(json["quote_currency"], "cNGN");
    assert_eq!(json["rate"], "1.0");
    assert_eq!(json["inverse_rate"], "1.0");
    assert_eq!(json["source"], "fixed_peg");
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_single_pair_cngn_to_ngn() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=cNGN&to=NGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["pair"], "cNGN/NGN");
    assert_eq!(json["rate"], "1.0");
}

#[tokio::test]
async fn test_invalid_currency() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=XYZ&to=cNGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"]["code"], "INVALID_CURRENCY");
    assert!(json["error"]["supported_currencies"].is_array());
}

#[tokio::test]
async fn test_invalid_pair() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN&to=BTC")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"]["code"], "INVALID_PAIR");
    assert!(json["error"]["supported_pairs"].is_array());
}

#[tokio::test]
async fn test_multiple_pairs() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?pairs=NGN/cNGN,cNGN/NGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["rates"].is_array());
    assert_eq!(json["rates"].as_array().unwrap().len(), 2);
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_all_pairs() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["rates"].is_object());
    assert!(json["rates"]["NGN/cNGN"].is_object());
    assert!(json["rates"]["cNGN/NGN"].is_object());
    assert!(json["supported_currencies"].is_array());
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_cache_headers() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN&to=cNGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let headers = response.headers();
    assert!(headers.contains_key(header::CACHE_CONTROL));
    assert_eq!(
        headers.get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=30"
    );
    assert!(headers.contains_key(header::ETAG));
}

#[tokio::test]
async fn test_cors_headers() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN&to=cNGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let headers = response.headers();
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "*"
    );
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
}

#[tokio::test]
async fn test_options_preflight() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/rates")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let headers = response.headers();
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
}

#[tokio::test]
async fn test_response_format_single_pair() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN&to=cNGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Verify all required fields are present
    assert!(json["pair"].is_string());
    assert!(json["base_currency"].is_string());
    assert!(json["quote_currency"].is_string());
    assert!(json["rate"].is_string());
    assert!(json["inverse_rate"].is_string());
    assert!(json["spread_percentage"].is_string());
    assert!(json["last_updated"].is_string());
    assert!(json["source"].is_string());
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_inverse_rate_calculation() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN&to=cNGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // For 1:1 peg, inverse should also be 1.0
    assert_eq!(json["rate"], "1.0");
    assert_eq!(json["inverse_rate"], "1.0");
}

#[tokio::test]
async fn test_missing_parameters() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rates?from=NGN")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"]["code"], "INVALID_PARAMETERS");
}
