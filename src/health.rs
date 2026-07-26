//! Health check module
//! Provides health status for the application and its dependencies

use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{error, info};

use crate::cache::warmer::WarmingState;
use crate::cache::RedisCache;
use crate::chains::stellar::client::StellarClient;

/// Health status response
#[derive(Debug, Serialize, Clone)]
pub struct HealthStatus {
    pub status: HealthState,
    pub checks: HashMap<String, ComponentHealth>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Overall health state
#[derive(Debug, Serialize, Clone)]
pub enum HealthState {
    Healthy,
    /// Cache warming is still in progress. The service is otherwise healthy and
    /// DOES accept traffic — this is a degraded-but-serving state, distinct from
    /// `Unhealthy`, so load balancers don't restart the instance mid-warmup.
    Warming,
    Degraded,
    Unhealthy,
}

/// Individual component health status
#[derive(Debug, Serialize, Clone)]
pub struct ComponentHealth {
    pub status: ComponentState,
    pub response_time_ms: Option<u128>,
    pub details: Option<String>,
}

/// Component state
#[derive(Debug, Serialize, Clone)]
pub enum ComponentState {
    Up,
    Down,
    Warning,
}

impl HealthStatus {
    pub fn new() -> Self {
        Self {
            status: HealthState::Healthy,
            checks: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Whether the service should be considered up for load-balancer purposes.
    /// `Warming` counts as healthy: the service accepts traffic while cache
    /// warmup finishes in the background.
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, HealthState::Healthy | HealthState::Warming)
    }

    /// Merge cache-warming state into this health status. Escalates to
    /// `Warming` only when nothing else has already reported Degraded/Unhealthy,
    /// so a real dependency failure during warmup is never masked.
    fn apply_warming(&mut self, ws: &WarmingState) {
        if !ws.is_ready() {
            let pct = ws.progress_pct();
            self.checks.insert(
                "cache_warming".to_string(),
                ComponentHealth::warning(
                    None,
                    Some(format!("Cache warming in progress ({}%)", pct)),
                ),
            );
            if matches!(self.status, HealthState::Healthy) {
                self.status = HealthState::Warming;
            }
        } else {
            self.checks
                .insert("cache_warming".to_string(), ComponentHealth::up(None));
        }
    }
}

impl ComponentHealth {
    pub fn up(response_time_ms: Option<u128>) -> Self {
        Self {
            status: ComponentState::Up,
            response_time_ms,
            details: None,
        }
    }

    pub fn down(details: Option<String>) -> Self {
        Self {
            status: ComponentState::Down,
            response_time_ms: None,
            details,
        }
    }

    pub fn warning(response_time_ms: Option<u128>, details: Option<String>) -> Self {
        Self {
            status: ComponentState::Warning,
            response_time_ms,
            details,
        }
    }
}

/// Health checker for the application
#[derive(Clone)]
pub struct HealthChecker {
    db_pool: Option<sqlx::PgPool>,
    cache: Option<RedisCache>,
    stellar_client: Option<StellarClient>,
    /// Reports `Warming` (not `Unhealthy`) with progress until cache warming
    /// completes — the service still accepts traffic throughout.
    pub warming_state: Option<WarmingState>,
    /// Optional replication monitor for circuit-breaker-aware lag checks.
    pub replication_monitor: Option<crate::database::replication_monitor::ReplicationMonitor>,
}

impl HealthChecker {
    pub fn new(
        db_pool: Option<sqlx::PgPool>,
        cache: Option<RedisCache>,
        stellar_client: Option<StellarClient>,
    ) -> Self {
        Self {
            db_pool,
            cache,
            stellar_client,
            warming_state: None,
            replication_monitor: None,
        }
    }

    /// Attach a warming state so health checks report `Warming` (with progress)
    /// instead of `Unhealthy` while cache warming is still in progress.
    pub fn with_warming_state(mut self, state: WarmingState) -> Self {
        self.warming_state = Some(state);
        self
    }

    /// Attach a replication monitor for circuit-breaker-aware lag checks.
    pub fn with_replication_monitor(
        mut self,
        monitor: crate::database::replication_monitor::ReplicationMonitor,
    ) -> Self {
        self.replication_monitor = Some(monitor);
        self
    }

    /// Perform comprehensive health check
    pub async fn check_health(&self) -> HealthStatus {
        let mut health_status = HealthStatus::new();
        let mut overall_healthy = true;
        let mut any_disabled = false;

        // Check database health
        if let Some(db_pool) = &self.db_pool {
            match timeout(Duration::from_secs(5), check_database_health(db_pool)).await {
                Ok(db_result) => match db_result {
                    Ok(response_time) => {
                        health_status.checks.insert(
                            "database".to_string(),
                            ComponentHealth::up(Some(response_time)),
                        );
                        info!("Database health check: OK ({}ms)", response_time);
                    }
                    Err(e) => {
                        overall_healthy = false;
                        health_status.checks.insert(
                            "database".to_string(),
                            ComponentHealth::down(Some(e.to_string())),
                        );
                        error!("Database health check failed: {}", e);
                    }
                },
                Err(_) => {
                    overall_healthy = false;
                    health_status.checks.insert(
                        "database".to_string(),
                        ComponentHealth::down(Some("Timeout".to_string())),
                    );
                    error!("Database health check timed out");
                }
            }
        } else {
            any_disabled = true;
            health_status.checks.insert(
                "database".to_string(),
                ComponentHealth::warning(None, Some("Disabled by configuration".to_string())),
            );
        }

        // Check cache health
        if let Some(cache) = &self.cache {
            match timeout(Duration::from_secs(5), check_cache_health(cache)).await {
                Ok(cache_result) => match cache_result {
                    Ok(response_time) => {
                        health_status.checks.insert(
                            "cache".to_string(),
                            ComponentHealth::up(Some(response_time)),
                        );
                        info!("Cache health check: OK ({}ms)", response_time);
                    }
                    Err(e) => {
                        overall_healthy = false;
                        health_status.checks.insert(
                            "cache".to_string(),
                            ComponentHealth::down(Some(e.to_string())),
                        );
                        error!("Cache health check failed: {}", e);
                    }
                },
                Err(_) => {
                    overall_healthy = false;
                    health_status.checks.insert(
                        "cache".to_string(),
                        ComponentHealth::down(Some("Timeout".to_string())),
                    );
                    error!("Cache health check timed out");
                }
            }
        } else {
            any_disabled = true;
            health_status.checks.insert(
                "cache".to_string(),
                ComponentHealth::warning(None, Some("Disabled by configuration".to_string())),
            );
        }

        // Check Stellar health
        if let Some(stellar_client) = &self.stellar_client {
            match timeout(
                Duration::from_secs(10),
                check_stellar_health(stellar_client),
            )
            .await
            {
                Ok(stellar_result) => match stellar_result {
                    Ok(response_time) => {
                        health_status.checks.insert(
                            "stellar".to_string(),
                            ComponentHealth::up(Some(response_time)),
                        );
                        info!("Stellar health check: OK ({}ms)", response_time);
                    }
                    Err(e) => {
                        overall_healthy = false;
                        health_status.checks.insert(
                            "stellar".to_string(),
                            ComponentHealth::down(Some(e.to_string())),
                        );
                        error!("Stellar health check failed: {}", e);
                    }
                },
                Err(_) => {
                    overall_healthy = false;
                    health_status.checks.insert(
                        "stellar".to_string(),
                        ComponentHealth::down(Some("Timeout".to_string())),
                    );
                    error!("Stellar health check timed out");
                }
            }
        } else {
            any_disabled = true;
            health_status.checks.insert(
                "stellar".to_string(),
                ComponentHealth::warning(None, Some("Disabled by configuration".to_string())),
            );
        }

        // Set overall status
        health_status.status = if overall_healthy {
            if any_disabled {
                HealthState::Degraded
            } else {
                HealthState::Healthy
            }
        } else {
            HealthState::Unhealthy
        };

        // Cache warming: reported as a degraded-but-serving `Warming` state, not
        // `Unhealthy` — the server accepts traffic immediately on startup and
        // warms caches in the background (Issue: premature LB restarts during
        // warmup).
        if let Some(ref ws) = self.warming_state {
            health_status.apply_warming(ws);
        }

        health_status
    }
}

// Add a function to check database health
pub async fn check_database_health(
    pool: &sqlx::PgPool,
) -> Result<u128, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();

    // Try to perform a simple query to check database connectivity
    match sqlx::query("SELECT 1").fetch_one(pool).await {
        Ok(_) => Ok(start.elapsed().as_millis()),
        Err(e) => Err(Box::new(e)),
    }
}

// Add a function to check cache health
pub async fn check_cache_health(
    cache: &RedisCache,
) -> Result<u128, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();

    // Try to ping the cache to check connectivity
    match cache.get_connection().await {
        Ok(mut conn) => {
            let result: redis::RedisResult<String> =
                redis::cmd("PING").query_async(&mut *conn).await;
            match result {
                Ok(_) => Ok(start.elapsed().as_millis()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            }
        }
        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
    }
}

// Add a function to check Stellar health
pub async fn check_stellar_health(
    stellar_client: &crate::chains::stellar::client::StellarClient,
) -> Result<u128, Box<dyn std::error::Error + Send + Sync>> {
    // Try to perform a simple operation to check Stellar connectivity
    match stellar_client.health_check().await {
        Ok(status) => {
            if status.is_healthy {
                Ok(status.response_time_ms as u128)
            } else {
                Err("Stellar service unhealthy".into())
            }
        }
        Err(e) => Err(Box::new(e)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_status_creation() {
        let health_status = HealthStatus::new();
        assert!(matches!(health_status.status, HealthState::Healthy));
        assert!(health_status.checks.is_empty());
        assert!(health_status.timestamp <= chrono::Utc::now());
    }

    #[test]
    fn test_component_health_states() {
        let up_health = ComponentHealth::up(Some(100));
        assert!(matches!(up_health.status, ComponentState::Up));
        assert_eq!(up_health.response_time_ms, Some(100));

        let down_health = ComponentHealth::down(Some("Test error".to_string()));
        assert!(matches!(down_health.status, ComponentState::Down));
        assert_eq!(down_health.details, Some("Test error".to_string()));

        let warning_health = ComponentHealth::warning(Some(500), Some("Slow response".to_string()));
        assert!(matches!(warning_health.status, ComponentState::Warning));
        assert_eq!(warning_health.response_time_ms, Some(500));
        assert_eq!(warning_health.details, Some("Slow response".to_string()));
    }

    #[test]
    fn test_warming_reports_degraded_not_unhealthy() {
        // Regression test: cache warming in progress must never force
        // Unhealthy — that causes premature load-balancer restarts.
        let ws = WarmingState::new();
        let mut status = HealthStatus::new();
        status.apply_warming(&ws);

        assert!(matches!(status.status, HealthState::Warming));
        assert!(status.is_healthy(), "Warming must still accept traffic");
        let check = status.checks.get("cache_warming").unwrap();
        assert!(matches!(check.status, ComponentState::Warning));
        assert!(check.details.as_ref().unwrap().contains("0%"));
    }

    #[test]
    fn test_warming_does_not_mask_existing_unhealthy_status() {
        let ws = WarmingState::new();
        let mut status = HealthStatus::new();
        status.status = HealthState::Unhealthy;
        status.apply_warming(&ws);

        // A real dependency failure must not be overwritten by the warming state.
        assert!(matches!(status.status, HealthState::Unhealthy));
    }

    #[test]
    fn test_warming_ready_reports_healthy() {
        let _ = crate::metrics::registry(); // mark_ready() publishes the progress gauge
        let ws = WarmingState::new();
        ws.mark_ready();
        let mut status = HealthStatus::new();
        status.apply_warming(&ws);

        assert!(matches!(status.status, HealthState::Healthy));
        let check = status.checks.get("cache_warming").unwrap();
        assert!(matches!(check.status, ComponentState::Up));
    }
}

// ---------------------------------------------------------------------------
// Edge / replica health (Issue #348)
// ---------------------------------------------------------------------------

/// Maximum tolerated replication lag before the health endpoint returns 503.
/// DNS failover is triggered when this threshold is breached.
pub const REPLICATION_LAG_THRESHOLD_SECS: i64 = 5;

/// Check replication lag on the read replica.
///
/// Queries `pg_stat_replication` on the primary (via `DATABASE_URL`) and
/// returns the lag in seconds.  Returns `None` when no replica is configured.
pub async fn check_replication_lag(
    primary_pool: &sqlx::PgPool,
) -> Result<Option<i64>, Box<dyn std::error::Error + Send + Sync>> {
    // pg_stat_replication is only populated on the primary.
    let row: Option<(Option<f64>,)> = sqlx::query_as(
        "SELECT EXTRACT(EPOCH FROM write_lag)::float8 \
         FROM pg_stat_replication \
         ORDER BY write_lag DESC NULLS LAST \
         LIMIT 1",
    )
    .fetch_optional(primary_pool)
    .await?;

    Ok(row.and_then(|(lag,)| lag.map(|l| l as i64)))
}

/// Axum handler: `GET /health/edge`
///
/// Returns 200 when the gateway is healthy and replication lag is within
/// threshold.  Returns 503 when lag exceeds `REPLICATION_LAG_THRESHOLD_SECS`,
/// signalling the DNS load balancer to fail over to the next closest region.
pub async fn edge_health_handler(
    axum::extract::State(checker): axum::extract::State<std::sync::Arc<HealthChecker>>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use serde_json::json;

    let region = crate::gateway::region::current_region();

    // Run the standard health check first.
    let status = checker.check_health().await;
    if !status.is_healthy() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({
                "status": "unhealthy",
                "region": region,
                "reason": "dependency_failure"
            })),
        );
    }

    // Use the ReplicationMonitor if available; fall back to a direct query.
    if let Some(monitor) = &checker.replication_monitor {
        let lag_secs = monitor.lag_secs();
        let breaker_open = monitor.is_open();

        // Update circuit-breaker metric on every health probe.
        crate::database::metrics::set_circuit_breaker(
            &crate::gateway::region::current_region(),
            breaker_open,
        );

        if lag_secs > REPLICATION_LAG_THRESHOLD_SECS || breaker_open {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({
                    "status": "unhealthy",
                    "region": region,
                    "reason": "replication_lag",
                    "lag_secs": lag_secs,
                    "threshold_secs": REPLICATION_LAG_THRESHOLD_SECS,
                    "circuit_breaker_open": breaker_open
                })),
            );
        }

        return (
            StatusCode::OK,
            axum::Json(json!({
                "status": "healthy",
                "region": region,
                "replication_lag_secs": lag_secs,
                "circuit_breaker_open": breaker_open
            })),
        );
    }

    // Fallback: direct pg_stat_replication query.
    if let Some(pool) = &checker.db_pool {
        match check_replication_lag(pool).await {
            Ok(Some(lag)) if lag > REPLICATION_LAG_THRESHOLD_SECS => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(json!({
                        "status": "unhealthy",
                        "region": region,
                        "reason": "replication_lag",
                        "lag_secs": lag,
                        "threshold_secs": REPLICATION_LAG_THRESHOLD_SECS
                    })),
                );
            }
            Ok(lag) => {
                return (
                    StatusCode::OK,
                    axum::Json(json!({
                        "status": "healthy",
                        "region": region,
                        "replication_lag_secs": lag
                    })),
                );
            }
            Err(e) => {
                tracing::warn!("Could not query replication lag: {e}");
            }
        }
    }

    (
        StatusCode::OK,
        axum::Json(json!({ "status": "healthy", "region": region })),
    )
}
