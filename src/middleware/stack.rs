//! Middleware stack configuration
//!
//! This module configures the middleware stack for the application.

use axum::Router;
use tower::ServiceBuilder;
use tower_http::request_id::{PropagateRequestIdLayer, SetRequestIdLayer};
use tracing::info;

use crate::middleware::{
    logging::{request_logging_middleware, UuidRequestId},
    metrics::metrics_middleware,
    cors::{cors_middleware, CorsConfig},
    security::security_headers_middleware,
    edge_cache::edge_cache_middleware,
    sanctions::sanctions_middleware,
    replay_prevention::replay_prevention_middleware,
    rate_limit::rate_limit_middleware,
    api_key::api_key_middleware,
};

/// Middleware stack configuration
#[derive(Clone, Debug)]
pub struct MiddlewareConfig {
    pub enable_logging: bool,
    pub enable_metrics: bool,
    pub enable_cors: bool,
    pub enable_security_headers: bool,
    pub enable_edge_cache: bool,
    pub enable_sanctions: bool,
    pub enable_replay_prevention: bool,
    pub enable_rate_limit: bool,
    pub enable_api_key_auth: bool,
    pub cors_config: Option<CorsConfig>,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            enable_logging: true,
            enable_metrics: true,
            enable_cors: true,
            enable_security_headers: true,
            enable_edge_cache: true,
            enable_sanctions: true,
            enable_replay_prevention: true,
            enable_rate_limit: true,
            enable_api_key_auth: true,
            cors_config: None,
        }
    }
}

impl MiddlewareConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            enable_logging: std::env::var("ENABLE_LOGGING_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_metrics: std::env::var("ENABLE_METRICS_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_cors: std::env::var("ENABLE_CORS_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_security_headers: std::env::var("ENABLE_SECURITY_HEADERS_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_edge_cache: std::env::var("ENABLE_EDGE_CACHE_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_sanctions: std::env::var("ENABLE_SANCTIONS_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_replay_prevention: std::env::var("ENABLE_REPLAY_PREVENTION_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_rate_limit: std::env::var("ENABLE_RATE_LIMIT_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            enable_api_key_auth: std::env::var("ENABLE_API_KEY_AUTH_MIDDLEWARE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            cors_config: Some(CorsConfig::from_env()),
        }
    }
}

/// Apply middleware stack to router
pub fn apply_middleware_stack(router: Router, config: &MiddlewareConfig) -> Router {
    info!("Applying middleware stack...");
    
    let mut service_builder = ServiceBuilder::new();
    
    // Request ID layer (always applied)
    service_builder = service_builder
        .layer(SetRequestIdLayer::x_request_id(UuidRequestId))
        .layer(PropagateRequestIdLayer::x_request_id());
    
    // Apply configured middleware
    if config.enable_logging {
        info!("Applying logging middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(request_logging_middleware));
    }
    
    if config.enable_metrics {
        info!("Applying metrics middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(metrics_middleware));
    }
    
    if config.enable_security_headers {
        info!("Applying security headers middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(security_headers_middleware));
    }
    
    if config.enable_edge_cache {
        info!("Applying edge cache middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(edge_cache_middleware));
    }
    
    if config.enable_sanctions {
        info!("Applying sanctions middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(sanctions_middleware));
    }
    
    if config.enable_replay_prevention {
        info!("Applying replay prevention middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(replay_prevention_middleware));
    }
    
    if config.enable_rate_limit {
        info!("Applying rate limit middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(rate_limit_middleware));
    }
    
    if config.enable_api_key_auth {
        info!("Applying API key authentication middleware");
        service_builder = service_builder.layer(axum::middleware::from_fn(api_key_middleware));
    }
    
    // Apply CORS middleware if enabled
    let router = if config.enable_cors {
        info!("Applying CORS middleware");
        if let Some(cors_config) = &config.cors_config {
            router.layer(axum::middleware::from_fn_with_state(
                cors_config.clone(),
                cors_middleware,
            ))
        } else {
            router
        }
    } else {
        router
    };
    
    // Apply service builder layers
    let router = router.layer(service_builder);
    
    info!("Middleware stack applied successfully");
    router
}