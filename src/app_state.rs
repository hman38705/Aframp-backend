//! Application state initialization
//!
//! This module handles initialization of shared application state including
//! database connections, cache pools, service instances, and configuration.

use std::sync::Arc;
use tracing::info;

use crate::{
    config::AppConfig,
    database::{init_pool, PoolConfig},
    cache::{init_cache_pool, CacheConfig, RedisCache},
    chains::stellar::{client::StellarClient, config::StellarConfig},
    security::{AnomalyDetectionService, CircuitBreakerMiddleware, AnomalyDetectionConfig},
    payments::factory::PaymentProviderFactory,
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db_pool: Option<Arc<crate::database::Pool>>,
    pub redis_cache: Option<Arc<RedisCache>>,
    pub stellar_client: Option<Arc<StellarClient>>,
    pub anomaly_service: Option<Arc<AnomalyDetectionService>>,
    pub circuit_breaker: Option<Arc<CircuitBreakerMiddleware>>,
    pub payment_provider_factory: Option<Arc<PaymentProviderFactory>>,
}

/// Initialize application state
pub async fn init_app_state(config: AppConfig) -> anyhow::Result<AppState> {
    info!("Initializing application state...");
    
    let app_config = Arc::new(config);
    
    // Initialize database pool
    let db_pool = init_database_pool(&app_config).await?;
    
    // Initialize cache pool
    let redis_cache = init_cache_pool(&app_config).await?;
    
    // Initialize Stellar client
    let stellar_client = init_stellar_client(&app_config).await?;
    
    // Initialize security services
    let (anomaly_service, circuit_breaker) = init_security_services(&db_pool, &app_config).await?;
    
    // Initialize payment provider factory
    let payment_provider_factory = init_payment_providers(&app_config).await?;
    
    Ok(AppState {
        config: app_config,
        db_pool,
        redis_cache,
        stellar_client,
        anomaly_service,
        circuit_breaker,
        payment_provider_factory,
    })
}

/// Initialize database connection pool
async fn init_database_pool(config: &Arc<AppConfig>) -> anyhow::Result<Option<Arc<crate::database::Pool>>> {
    let skip_externals = std::env::var("SKIP_EXTERNALS")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";
    
    if skip_externals {
        info!("⏭️  Skipping database initialization (SKIP_EXTERNALS=true)");
        return Ok(None);
    }
    
    info!("📊 Initializing database connection pool...");
    
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?;
    
    let db_pool_config = PoolConfig {
        max_connections: std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20),
        min_connections: std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
        connection_timeout: std::time::Duration::from_secs(
            std::env::var("DB_CONNECTION_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        ),
        idle_timeout: std::time::Duration::from_secs(
            std::env::var("DB_IDLE_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(600),
        ),
        max_lifetime: std::time::Duration::from_secs(
            std::env::var("DB_MAX_LIFETIME")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1800),
        ),
    };
    
    let db_pool = init_pool(&database_url, Some(db_pool_config.clone()))
        .await
        .map_err(|e| {
            info!("Failed to initialize database pool: {}", e);
            e
        })?;
    
    info!(
        max_connections = db_pool.options().get_max_connections(),
        "✅ Database connection pool initialized"
    );
    
    Ok(Some(Arc::new(db_pool)))
}

/// Initialize Redis cache pool
async fn init_cache_pool(config: &Arc<AppConfig>) -> anyhow::Result<Option<Arc<RedisCache>>> {
    let skip_externals = std::env::var("SKIP_EXTERNALS")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";
    
    if skip_externals {
        info!("⏭️  Skipping Redis initialization (SKIP_EXTERNALS=true)");
        return Ok(None);
    }
    
    info!("🔄 Initializing Redis cache connection pool...");
    
    let redis_url = std::env::var("REDIS_URL")
        .map_err(|_| anyhow::anyhow!("REDIS_URL not set"))?;
    
    let cache_config = CacheConfig {
        redis_url: redis_url.clone(),
        max_connections: std::env::var("CACHE_MAX_CONNECTIONS")
            .or_else(|_| std::env::var("REDIS_MAX_CONNECTIONS"))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20),
        min_idle: std::env::var("REDIS_MIN_IDLE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
        connection_timeout: std::time::Duration::from_secs(
            std::env::var("REDIS_CONNECTION_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
        ),
        max_lifetime: std::time::Duration::from_secs(
            std::env::var("REDIS_MAX_LIFETIME")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        ),
        idle_timeout: std::time::Duration::from_secs(
            std::env::var("REDIS_IDLE_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
        ),
        health_check_interval: std::time::Duration::from_secs(
            std::env::var("REDIS_HEALTH_CHECK_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        ),
    };
    
    let cache_pool = init_cache_pool(cache_config)
        .await
        .map_err(|e| {
            info!("Failed to initialize cache pool: {}", e);
            e
        })?;
    
    let redis_cache = RedisCache::new(cache_pool);
    info!(redis_url = %redis_url, "✅ Cache connection pool initialized");
    
    Ok(Some(Arc::new(redis_cache)))
}

/// Initialize Stellar client
async fn init_stellar_client(config: &Arc<AppConfig>) -> anyhow::Result<Option<Arc<StellarClient>>> {
    let skip_externals = std::env::var("SKIP_EXTERNALS")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";
    
    if skip_externals {
        info!("⏭️  Skipping Stellar initialization (SKIP_EXTERNALS=true)");
        return Ok(None);
    }
    
    info!("⭐ Initializing Stellar client...");
    
    let stellar_config = StellarConfig::from_env()
        .map_err(|e| {
            info!("❌ Failed to load Stellar configuration: {}", e);
            e
        })?;
    
    let stellar_client = StellarClient::new(stellar_config)
        .map_err(|e| {
            info!("❌ Failed to initialize Stellar client: {}", e);
            e
        })?;
    
    info!("✅ Stellar client initialized successfully");
    
    Ok(Some(Arc::new(stellar_client)))
}

/// Initialize security services
async fn init_security_services(
    db_pool: &Option<Arc<crate::database::Pool>>,
    config: &Arc<AppConfig>,
) -> anyhow::Result<(Option<Arc<AnomalyDetectionService>>, Option<Arc<CircuitBreakerMiddleware>>)> {
    let (anomaly_service, circuit_breaker) = if let Some(ref pool) = db_pool {
        let sec_config = AnomalyDetectionConfig::from_env();
        let service = Arc::new(AnomalyDetectionService::new(
            pool.clone(),
            sec_config,
        ));
        let middleware = Arc::new(CircuitBreakerMiddleware::new(
            service.clone(),
        ));
        info!("✅ AnomalyDetectionService and CircuitBreakerMiddleware initialized");
        (Some(service), Some(middleware))
    } else {
        (None, None)
    };
    
    Ok((anomaly_service, circuit_breaker))
}

/// Initialize payment provider factory
async fn init_payment_providers(config: &Arc<AppConfig>) -> anyhow::Result<Option<Arc<PaymentProviderFactory>>> {
    let payment_provider_factory = PaymentProviderFactory::from_env()
        .map_err(|e| {
            info!("Failed to initialize payment provider factory: {}", e);
            anyhow::anyhow!("Cannot start without payment providers: {}", e)
        })?;
    
    info!("✅ Payment provider factory initialized");
    Ok(Some(Arc::new(payment_provider_factory)))
}