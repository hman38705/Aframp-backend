//! Rates API Demo
//!
//! Demonstrates how to set up and use the rates API endpoint.
//! Shows integration with exchange rate service and caching.

use aframp_backend::api::rates::{get_rates, options_rates, RatesState};
use aframp_backend::cache::cache::RedisCache;
use aframp_backend::cache::{init_cache_pool, CacheConfig};
use aframp_backend::database::connection::create_pool;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use aframp_backend::services::exchange_rate::{ExchangeRateService, ExchangeRateServiceConfig};
use axum::{
    routing::{get, options},
    Router,
};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Rates API Demo");

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/aframp".to_string());
    let db_pool = create_pool(&database_url).await?;
    info!("Database connected");

    // Initialize cache (optional)
    let cache = if let Ok(redis_url) = std::env::var("REDIS_URL") {
        let cache_config = CacheConfig {
            url: redis_url,
            max_connections: 10,
            min_idle: 2,
            connection_timeout_seconds: 5,
            max_lifetime_seconds: Some(300),
        };
        let cache_pool = init_cache_pool(cache_config).await?;
        Some(Arc::new(RedisCache::new(cache_pool)))
    } else {
        info!("Redis not configured, running without cache");
        None
    };

    // Initialize exchange rate service
    let repository = ExchangeRateRepository::new(db_pool.clone());
    let config = ExchangeRateServiceConfig::default();
    let mut exchange_rate_service = ExchangeRateService::new(repository, config);

    if let Some(ref cache_instance) = cache {
        exchange_rate_service = exchange_rate_service.with_cache((**cache_instance).clone());
    }

    let exchange_rate_service = Arc::new(exchange_rate_service);

    // Create rates state
    let rates_state = RatesState {
        exchange_rate_service,
        cache,
    };

    // Build router
    let app = Router::new()
        .route("/api/rates", get(get_rates).options(options_rates))
        .with_state(rates_state);

    // Start server
    let addr = "0.0.0.0:3000";
    info!("Rates API listening on http://{}", addr);
    info!("");
    info!("Try these endpoints:");
    info!("  GET http://localhost:3000/api/rates?from=NGN&to=cNGN");
    info!("  GET http://localhost:3000/api/rates?pairs=NGN/cNGN,cNGN/NGN");
    info!("  GET http://localhost:3000/api/rates");
    info!("");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
