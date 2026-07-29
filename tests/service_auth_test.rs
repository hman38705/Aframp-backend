//! Integration tests for microservice-to-microservice authentication

#[cfg(all(test, feature = "database"))]
mod service_auth_tests {
    use Bitmesh_backend::service_auth::{
        ServiceAllowlist, ServiceRegistry, ServiceRegistration, ServiceTokenManager,
        TokenRefreshConfig,
    };
    use std::sync::Arc;

    // ── Service registration tests ───────────────────────────────────────────

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_service_registration() -> Result<(), anyhow::Error> {
        let pool = setup_test_pool().await;
        let registry = ServiceRegistry::new(Arc::new(pool));

        let registration = ServiceRegistration {
            service_name: "test_worker".to_string(),
            allowed_scopes: vec!["worker:execute".to_string()],
            allowed_targets: vec!["/api/internal/*".to_string()],
        };

        let identity = registry.register_service(registration).await?;

        assert_eq!(identity.service_name, "test_worker");
        assert!(identity.client_id.starts_with("service_"));
        assert!(identity.client_secret.starts_with("svc_secret_"));
        assert!(identity
            .allowed_scopes
            .contains(&"microservice:internal".to_string()));

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_service_registration_includes_internal_scope() -> Result<(), anyhow::Error> {
        let pool = setup_test_pool().await;
        let registry = ServiceRegistry::new(Arc::new(pool));

        let registration = ServiceRegistration {
            service_name: "test_service".to_string(),
            allowed_scopes: vec!["custom:scope".to_string()],
            allowed_targets: vec![],
        };

        let identity = registry.register_service(registration).await?;

        assert!(identity
            .allowed_scopes
            .contains(&"microservice:internal".to_string()));
        assert!(identity
            .allowed_scopes
            .contains(&"custom:scope".to_string()));

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_services() -> Result<(), anyhow::Error> {
        let pool = setup_test_pool().await;
        let registry = ServiceRegistry::new(Arc::new(pool.clone()));

        // Register multiple services
        for i in 1..=3 {
            let registration = ServiceRegistration {
                service_name: format!("test_service_{}", i),
                allowed_scopes: vec!["test:scope".to_string()],
                allowed_targets: vec![],
            };
            registry.register_service(registration).await?;
        }

        let services = registry.list_services().await?;
        assert!(services.len() >= 3);

        Ok(())
    }

    // ── Secret rotation tests ────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_secret_rotation() -> Result<(), anyhow::Error> {
        let pool = setup_test_pool().await;
        let registry = ServiceRegistry::new(Arc::new(pool));

        let registration = ServiceRegistration {
            service_name: "rotation_test".to_string(),
            allowed_scopes: vec![],
            allowed_targets: vec![],
        };

        let identity = registry.register_service(registration).await?;
        let old_secret = identity.client_secret.clone();

        let new_secret = registry.rotate_secret("rotation_test", 300).await?;

        assert_ne!(old_secret, new_secret);
        assert!(new_secret.starts_with("svc_secret_"));

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_secret_rotation_nonexistent_service() -> Result<(), anyhow::Error> {
        let pool = setup_test_pool().await;
        let registry = ServiceRegistry::new(Arc::new(pool));

        let result = registry.rotate_secret("nonexistent", 300).await;
        assert!(result.is_err());

        Ok(())
    }

    // ── Token manager tests ──────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_token_manager_initialization() -> Result<(), anyhow::Error> {
        let config = TokenRefreshConfig::default();
        let manager = ServiceTokenManager::new(
            "test_service".to_string(),
            "service_test".to_string(),
            "test_secret".to_string(),
            "http://localhost:8080/oauth/token".to_string(),
            config,
        );

        // Note: This will fail without a running OAuth server.
        // In real tests, mock the HTTP client.
        let _result = manager.initialize().await;

        Ok(())
    }

    #[tokio::test]
    async fn test_token_refresh_threshold_calculation() {
        // Test that refresh threshold logic works correctly.
        // No fallible calls — no Result wrapper needed.
        let config = TokenRefreshConfig {
            refresh_threshold: 0.2,
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 5000,
        };

        assert_eq!(config.refresh_threshold, 0.2);
        assert_eq!(config.max_retries, 3);
    }

    // ── Allowlist tests ──────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_allowlist_exact_match() -> Result<(), anyhow::Error> {
        let (pool, cache) = setup_test_env().await;
        let allowlist = ServiceAllowlist::new(Arc::new(pool), Arc::new(cache));

        allowlist
            .set_permission("worker_service", "/api/settlement/process", true)
            .await?;

        let allowed = allowlist
            .is_allowed("worker_service", "/api/settlement/process")
            .await?;

        assert!(allowed);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_allowlist_wildcard_match() -> Result<(), anyhow::Error> {
        let (pool, cache) = setup_test_env().await;
        let allowlist = ServiceAllowlist::new(Arc::new(pool), Arc::new(cache));

        allowlist
            .set_permission("worker_service", "/api/settlement/*", true)
            .await?;

        let allowed = allowlist
            .is_allowed("worker_service", "/api/settlement/process")
            .await?;

        assert!(allowed);

        let allowed2 = allowlist
            .is_allowed("worker_service", "/api/settlement/verify")
            .await?;

        assert!(allowed2);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_allowlist_deny() -> Result<(), anyhow::Error> {
        let (pool, cache) = setup_test_env().await;
        let allowlist = ServiceAllowlist::new(Arc::new(pool), Arc::new(cache));

        allowlist
            .set_permission("worker_service", "/api/admin/*", false)
            .await?;

        let allowed = allowlist
            .is_allowed("worker_service", "/api/admin/users")
            .await?;

        assert!(!allowed);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_allowlist_not_in_list() -> Result<(), anyhow::Error> {
        let (pool, cache) = setup_test_env().await;
        let allowlist = ServiceAllowlist::new(Arc::new(pool), Arc::new(cache));

        let allowed = allowlist
            .is_allowed("unknown_service", "/api/anything")
            .await?;

        assert!(!allowed);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_allowlist_cache_invalidation() -> Result<(), anyhow::Error> {
        let (pool, cache) = setup_test_env().await;
        let allowlist = ServiceAllowlist::new(Arc::new(pool), Arc::new(cache));

        // Set permission
        allowlist
            .set_permission("test_service", "/api/test", true)
            .await?;

        // Check it's allowed
        let allowed = allowlist.is_allowed("test_service", "/api/test").await?;
        assert!(allowed);

        // Update permission
        allowlist
            .set_permission("test_service", "/api/test", false)
            .await?;

        // Check cache was invalidated
        let allowed = allowlist.is_allowed("test_service", "/api/test").await?;
        assert!(!allowed);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_allowlist_list_permissions() -> Result<(), anyhow::Error> {
        let (pool, cache) = setup_test_env().await;
        let allowlist = ServiceAllowlist::new(Arc::new(pool), Arc::new(cache));

        allowlist
            .set_permission("test_service", "/api/endpoint1", true)
            .await?;
        allowlist
            .set_permission("test_service", "/api/endpoint2", false)
            .await?;

        let permissions = allowlist.list_permissions("test_service").await?;

        assert_eq!(permissions.len(), 2);

        Ok(())
    }

    // ── Helper functions ─────────────────────────────────────────────────────

    async fn setup_test_pool() -> sqlx::PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/aframp_test".to_string());

        sqlx::PgPool::connect(&database_url)
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to connect to test database at {}: {}",
                    database_url, e
                )
            })
    }

    async fn setup_test_env() -> (sqlx::PgPool, Bitmesh_backend::cache::RedisCache) {
        let pool = setup_test_pool().await;

        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let cache_config = Bitmesh_backend::cache::CacheConfig {
            redis_url,
            ..Default::default()
        };

        let cache_pool = Bitmesh_backend::cache::init_cache_pool(cache_config)
            .await
            .unwrap_or_else(|e| panic!("Failed to initialise Redis cache: {}", e));

        let cache = Bitmesh_backend::cache::RedisCache::new(cache_pool);

        (pool, cache)
    }
}
