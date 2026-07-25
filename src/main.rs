//! Aframp Backend - Main Entry Point
//!
//! Multi-region, edge-cached, globally distributed Rust/Axum backend
//! for the Aframp platform.

// Module declarations
mod api;
mod api_keys;
mod analytics;
mod app_state;
mod audit;
mod auth;
mod cache;
mod config;
mod config_validation;
mod corridors;
mod database;
mod developer_portal;
mod error;
mod health;
mod logging;
mod metrics;
mod middleware;
mod oauth;
mod oracle;
mod payments;
mod recurring;
mod routes;
mod services;
mod startup;
mod telemetry;
mod verification;
mod wallet;
mod wallet_provisioning;
mod workers;

// External imports
use dotenv::dotenv;
use tracing::{error, info};

/// Main entry point
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv().ok();
    
    // Load application configuration
    let app_config = load_app_config()?;
    
    // Validate production configuration
    validate_production_config()?;
    
    // Start the application
    match startup::start_app(app_config).await {
        Ok(_) => {
            info!("👋 Application shutdown completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("❌ Application failed to start: {}", e);
            Err(e)
        }
    }
}

/// Load application configuration from environment
fn load_app_config() -> anyhow::Result<config::AppConfig> {
    info!("📝 Loading application configuration...");
    
    let app_config = config::AppConfig::from_env().map_err(|e| {
        error!("❌ Failed to load application configuration: {}", e);
        anyhow::anyhow!("Configuration error: {}", e)
    })?;
    
    app_config.validate().map_err(|e| {
        error!("❌ Configuration validation failed: {}", e);
        anyhow::anyhow!("Configuration validation error: {}", e)
    })?;
    
    info!(
        version = env!("CARGO_PKG_VERSION"),
        environment = %app_config.telemetry.environment,
        service = %app_config.telemetry.service_name,
        "✅ Configuration loaded successfully"
    );
    
    Ok(app_config)
}

/// Validate production configuration
fn validate_production_config() -> anyhow::Result<()> {
    info!("🔍 Validating production configuration...");
    
    if let Err(e) = config_validation::validate_production_config() {
        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".into());
        if app_env != "development" {
            error!("❌ {}", e);
            std::process::exit(1);
        } else {
            info!("⚠️  Config warnings (non-fatal in development):\n{}", e);
        }
    }
    
    info!("✅ Production configuration validated");
    Ok(())
}