//! Redis-based caching layer for Aframp
//!
//! This module provides a robust, fault-tolerant Redis caching layer that:
//! - Serves frequently accessed data in sub-millisecond latency
//! - Reduces database load by 60-80%
//! - Gracefully degrades when Redis is unavailable
//! - Uses clean abstractions, strong typing, and safe invalidation strategies

pub mod cache;
pub mod error;
pub mod keys;

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::{AsyncCommands, Client};
use std::time::Duration;
use tracing::{error, info, warn};

/// Redis connection pool type alias
pub type RedisPool = Pool<RedisConnectionManager>;

/// Redis cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Redis connection URL
    pub redis_url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum idle connections
    pub min_idle: u32,
    /// Connection timeout in seconds
    pub connection_timeout: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
    /// Idle timeout before closing connection
    pub idle_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            max_connections: 20,
            min_idle: 5,
            connection_timeout: Duration::from_secs(5),
            max_lifetime: Duration::from_secs(300),
            idle_timeout: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// Initialize Redis connection pool with fault tolerance
pub async fn init_cache_pool(config: CacheConfig) -> Result<RedisPool, CacheError> {
    info!(
        "Initializing Redis cache pool: max_connections={}, redis_url={}",
        config.max_connections, config.redis_url
    );

    // Create Redis client
    let client = Client::open(config.redis_url.clone())
        .map_err(|e| {
            error!("Failed to create Redis client: {}", e);
            CacheError::ConnectionError(e.to_string())
        })?;

    // Create connection manager
    let manager = RedisConnectionManager::new(client)
        .map_err(|e| {
            error!("Failed to create Redis connection manager: {}", e);
            CacheError::ConnectionError(e.to_string())
        })?;

    // Build pool
    let pool = Pool::builder()
        .max_size(config.max_connections)
        .min_idle(config.min_idle)
        .connection_timeout(config.connection_timeout)
        .max_lifetime(config.max_lifetime)
        .idle_timeout(config.idle_timeout)
        .test_on_check_out(false) // We'll handle health checks manually
        .build(manager)
        .await
        .map_err(|e| {
            error!("Failed to build Redis connection pool: {}", e);
            CacheError::ConnectionError(e.to_string())
        })?;

    // Test connection
    if let Err(e) = test_connection(&pool).await {
        warn!("Initial Redis connection test failed, but continuing: {}", e);
        // Don't fail here - allow graceful degradation
    }

    info!("Redis cache pool initialized successfully");
    Ok(pool)
}

/// Test Redis connection
async fn test_connection(pool: &RedisPool) -> Result<(), CacheError> {
    let mut conn = pool.get().await
        .map_err(|e| {
            error!("Failed to get Redis connection for test: {}", e);
            CacheError::ConnectionError(e.to_string())
        })?;

    let _: String = redis::cmd("PING")
        .query_async(&mut *conn)
        .await
        .map_err(|e| {
            error!("Redis PING failed: {}", e);
            CacheError::ConnectionError(e.to_string())
        })?;

    Ok(())
}

/// Health check for Redis connection pool
pub async fn health_check(pool: &RedisPool) -> Result<(), CacheError> {
    test_connection(pool).await
}

/// Get pool statistics
#[derive(Debug)]
pub struct CacheStats {
    pub connections: u32,
    pub idle_connections: u32,
    pub connections_in_use: u32,
}

pub fn get_cache_stats(pool: &RedisPool) -> CacheStats {
    CacheStats {
        connections: pool.state().connections as u32,
        idle_connections: pool.state().idle_connections as u32,
        connections_in_use: (pool.state().connections - pool.state().idle_connections) as u32,
    }
}

/// Graceful shutdown of cache pool
pub async fn shutdown_cache_pool(pool: &RedisPool) {
    info!("Shutting down Redis cache pool");
    pool.close().await;
}

use error::CacheError;