//! Integration tests for POST /api/onramp/quote
//!
//! Requires: DATABASE_URL, REDIS_URL
//! Run with: cargo test onramp_quote -- --ignored

use Bitmesh_backend::cache::{init_cache_pool, CacheConfig, RedisCache};
use Bitmesh_backend::database::onramp_quote_repository::OnrampQuoteRepository;
use Bitmesh_backend::database::{
    exchange_rate_repository::ExchangeRateRepository,
    fee_structure_repository::FeeStructureRepository,
    init_pool,
};
use Bitmesh_backend::services::onramp_quote::{OnrampQuoteRequest, OnrampQuoteService};
use Bitmesh_backend::services::{
    exchange_rate::{ExchangeRateService, ExchangeRateServiceConfig},
    fee_structure::FeeStructureService,
    rate_providers::FixedRateProvider,
};
use std::sync::Arc;

async fn setup_service() -> OnrampQuoteService {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/aframp_test".to_string());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let pool = init_pool(&database_url, None).await.expect("DB init");
    let cache_config = CacheConfig {
        redis_url,
        ..Default::default()
    };
    let cache_pool = init_cache_pool(cache_config).await.expect("Redis init");
    let redis_cache = RedisCache::new(cache_pool);

    let rate_repo = ExchangeRateRepository::new(pool.clone());
    let fee_repo = FeeStructureRepository::new(pool.clone());
    let fee_service = Arc::new(FeeStructureService::new(fee_repo));

    let exchange_rate_service = Arc::new(
        ExchangeRateService::new(rate_repo, ExchangeRateServiceConfig::default())
            .with_cache(redis_cache)
            .add_provider(Arc::new(FixedRateProvider::new()))
            .with_fee_service(fee_service.clone()),
    );

    let quote_repo = OnrampQuoteRepository::new(pool);

    OnrampQuoteService::new(
        exchange_rate_service,
        fee_service,
        quote_repo,
        None,
        "GXXXXDEFAULTISSUERXXXX".to_string(),
    )
}

#[tokio::test]
#[ignore]
async fn test_onramp_quote_success() {
    let service = setup_service().await;

    let result = service
        .create_quote(OnrampQuoteRequest { amount_ngn: 50000 })
        .await;

    let response = result.expect("Quote creation should succeed");

    assert!(!response.quote_id.is_empty());
    assert_eq!(response.amount_ngn, 50000);
    assert!(response.exchange_rate > 0.0);
    assert!(response.gross_cngn > 0.0);
    assert!(response.fee_cngn >= 0.0);
    assert!(response.net_cngn > 0.0);
    assert!(response.net_cngn <= response.gross_cngn);
    assert!(!response.expires_at.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_onramp_quote_rejects_zero_amount() {
    let service = setup_service().await;

    let result = service
        .create_quote(OnrampQuoteRequest { amount_ngn: 0 })
        .await;

    assert!(result.is_err());
}
