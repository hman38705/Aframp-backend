//! Application startup and shutdown logic
//!
//! This module handles application startup, graceful shutdown, and signal handling.

use std::sync::Arc;
use std::time::Duration;
use tokio::{signal, sync::watch};
use tracing::{error, info};
use axum::Router;

use crate::{
    config::AppConfig,
    telemetry::tracer::{init_tracer, shutdown_tracer},
    app_state::AppState,
    routes::router::{build_router, AppRouterState},
    middleware::stack::{apply_middleware_stack, MiddlewareConfig},
};

/// Application startup configuration
pub struct StartupConfig {
    pub host: String,
    pub port: String,
    pub graceful_shutdown_timeout: Duration,
    pub enable_health_checks: bool,
    pub enable_warmup: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            host: std::env::var("SERVER_HOST")
                .or_else(|_| std::env::var("HOST"))
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("SERVER_PORT")
                .or_else(|_| std::env::var("PORT"))
                .unwrap_or_else(|_| "8000".to_string()),
            graceful_shutdown_timeout: Duration::from_secs(30),
            enable_health_checks: true,
            enable_warmup: true,
        }
    }
}

/// Initialize and start the application
pub async fn start_app(config: AppConfig) -> anyhow::Result<()> {
    info!("🚀 Starting Aframp backend service");
    
    // Initialize telemetry
    init_tracing(&config)?;
    
    // Initialize application state
    let app_state = crate::app_state::init_app_state(config).await?;
    
    // Build router state
    let router_state = AppRouterState {
        db_pool: app_state.db_pool.clone(),
        redis_cache: app_state.redis_cache.clone(),
        stellar_client: app_state.stellar_client.clone(),
    };
    
    // Build router
    let router = build_router(router_state);
    
    // Apply middleware stack
    let middleware_config = MiddlewareConfig::from_env();
    let router = apply_middleware_stack(router, &middleware_config);
    
    // Create startup configuration
    let startup_config = StartupConfig::default();
    
    // Start the server
    start_server(router, startup_config).await
}

/// Initialize tracing and telemetry
fn init_tracing(config: &AppConfig) -> anyhow::Result<()> {
    info!("📊 Initializing OpenTelemetry tracer...");
    
    init_tracer(&config.telemetry).map_err(|e| {
        error!("❌ Failed to initialise OpenTelemetry tracer: {}", e);
        anyhow::anyhow!("Tracer initialisation error: {}", e)
    })?;
    
    info!("✅ OpenTelemetry tracer initialized");
    Ok(())
}

/// Start the HTTP server
async fn start_server(router: Router, config: StartupConfig) -> anyhow::Result<()> {
    let addr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| {
            error!("Failed to parse server address: {}", e);
            anyhow::anyhow!("Invalid server address: {}", e)
        })?;
    
    info!("🌐 Server starting on http://{}", addr);
    
    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    
    // Spawn graceful shutdown handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal_with_notify(shutdown_tx_clone).await;
    });
    
    // Create server
    let server = axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.closed().await;
            info!("🔄 Graceful shutdown initiated");
        });
    
    // Run server
    match server.await {
        Ok(_) => {
            info!("👋 Server shutdown completed");
            Ok(())
        }
        Err(e) => {
            error!("❌ Server error: {}", e);
            Err(anyhow::anyhow!("Server error: {}", e))
        }
    }
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!("failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                error!("failed to install signal handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
}

/// Shutdown signal handler with notification
async fn shutdown_signal_with_notify(shutdown_tx: watch::Sender<bool>) {
    shutdown_signal().await;
    let _ = shutdown_tx.send(true);
}

/// Perform application warmup tasks
pub async fn warmup_app(state: &AppState) -> anyhow::Result<()> {
    info!("🔥 Performing application warmup...");
    
    // Warm up database connections
    if let Some(pool) = &state.db_pool {
        info!("Warming up database connections...");
        match pool.acquire().await {
            Ok(_) => info!("✅ Database connections warmed up"),
            Err(e) => warn!("Failed to warm up database connections: {}", e),
        }
    }
    
    // Warm up cache connections
    if let Some(cache) = &state.redis_cache {
        info!("Warming up cache connections...");
        match cache.ping().await {
            Ok(_) => info!("✅ Cache connections warmed up"),
            Err(e) => warn!("Failed to warm up cache connections: {}", e),
        }
    }
    
    // Warm up Stellar client
    if let Some(client) = &state.stellar_client {
        info!("Warming up Stellar client...");
        match client.health_check().await {
            Ok(health) if health.is_healthy => {
                info!(
                    response_time_ms = health.response_time_ms,
                    "✅ Stellar Horizon is healthy"
                );
            }
            Ok(health) => {
                warn!(
                    error = health.error_message.unwrap_or_default(),
                    "❌ Stellar Horizon health check failed"
                );
            }
            Err(e) => {
                warn!("Failed to warm up Stellar client: {}", e);
            }
        }
    }
    
    info!("✅ Application warmup completed");
    Ok(())
}