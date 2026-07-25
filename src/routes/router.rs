//! Main router configuration
//!
//! This module builds the main application router by combining all route modules.

use std::sync::Arc;
use axum::Router;
use tracing::info;

use crate::{
    database::Pool,
    cache::RedisCache,
    api::developer::keys::DeveloperKeysState,
    chains::stellar::client::StellarClient,
};

/// Application state shared across all routes
#[derive(Clone)]
pub struct AppRouterState {
    pub db_pool: Option<Arc<Pool>>,
    pub redis_cache: Option<Arc<RedisCache>>,
    pub stellar_client: Option<Arc<StellarClient>>,
}

/// Build the main application router
pub fn build_router(state: AppRouterState) -> Router {
    info!("Building main application router...");
    
    // Start with base routes
    let mut router = Router::new();
    
    // Add health routes
    router = add_health_routes(router, &state);
    
    // Add API routes
    router = add_api_routes(router, &state);
    
    // Add admin routes
    router = add_admin_routes(router, &state);
    
    // Add developer routes
    router = add_developer_routes(router, &state);
    
    // Add authentication routes
    router = add_auth_routes(router, &state);
    
    info!("Router built successfully");
    router
}

/// Add health check routes
fn add_health_routes(router: Router, state: &AppRouterState) -> Router {
    // TODO: Implement health route integration
    router
}

/// Add API routes (onramp, offramp, wallet, etc.)
fn add_api_routes(router: Router, state: &AppRouterState) -> Router {
    // TODO: Extract API route integration from main.rs
    router
}

/// Add admin routes
fn add_admin_routes(router: Router, state: &AppRouterState) -> Router {
    // TODO: Extract admin route integration from main.rs
    router
}

/// Add developer routes
fn add_developer_routes(router: Router, state: &AppRouterState) -> Router {
    // TODO: Extract developer route integration from main.rs
    router
}

/// Add authentication routes
fn add_auth_routes(router: Router, state: &AppRouterState) -> Router {
    // TODO: Extract auth route integration from main.rs
    router
}