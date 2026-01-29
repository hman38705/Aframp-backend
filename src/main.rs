mod cache;
mod chains;
mod database;
mod error;
use dotenv::dotenv;
use axum::{
    routing::{get, post},
    Router,
};
use cache::{init_cache_pool, CacheConfig, RedisCache};
use chains::stellar::client::StellarClient;
use chains::stellar::config::StellarConfig;
use database::{init_pool, PoolConfig};
use std::net::SocketAddr;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    dotenv().ok();
    info!("Starting Aframp backend service");

    // Initialize database connection pool
    let database_url =
        std::env::var("DATABASE_URL".to_string()).unwrap();
    let db_pool = init_pool(&database_url, Some(PoolConfig::default())).await?;
    info!("Database connection pool initialized");

    // Initialize cache connection pool
    let redis_url =
        std::env::var("REDIS_URL".to_string()).unwrap();
    let cache_config = CacheConfig {
        redis_url,
        ..Default::default()
    };
    let cache_pool = init_cache_pool(cache_config).await?;
    let redis_cache = RedisCache::new(cache_pool);
    info!("Cache connection pool initialized");

    // Initialize Stellar client
    let stellar_config = StellarConfig::from_env().map_err(|e| {
        error!("Failed to load Stellar configuration: {}", e);
        e
    })?;

    let stellar_client = StellarClient::new(stellar_config).map_err(|e| {
        error!("Failed to initialize Stellar client: {}", e);
        e
    })?;

    info!("Stellar client initialized successfully");

    // Health check Stellar
    let health_status = stellar_client.health_check().await?;
    if health_status.is_healthy {
        info!(
            "Stellar Horizon is healthy - Response time: {}ms",
            health_status.response_time_ms
        );
    } else {
        error!(
            "Stellar Horizon health check failed: {}",
            health_status
                .error_message
                .unwrap_or_else(|| "Unknown error".to_string())
        );
    }

    // Demo functionality
    info!("=== Demo: Testing Stellar functionality ===");
    let test_address = "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX";

    match stellar_client.account_exists(test_address).await {
        Ok(exists) => {
            if exists {
                info!("Account {} exists", test_address);
                match stellar_client.get_account(test_address).await {
                    Ok(account) => {
                        info!("Successfully fetched account details");
                        info!("Account ID: {}", account.account_id);
                        info!("Sequence: {}", account.sequence);
                        info!("Number of balances: {}", account.balances.len());
                        for balance in &account.balances {
                            info!("Balance: {} {}", balance.balance, balance.asset_type);
                        }
                    }
                    Err(e) => info!("Account exists but couldn't fetch details: {}", e),
                }
            } else {
                info!("Account {} does not exist (this is expected for test addresses)", test_address);
            }
        },
        Err(e) => info!("Error checking account existence (this is expected for non-existent test addresses): {}", e),
    }

    // Create the application router
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/stellar/account/:address", get(get_stellar_account))
        .with_state(AppState {
            db_pool,
            redis_cache,
            stellar_client,
        });

    // Run the server
    let host = std::env::var("HOST".to_string()).unwrap();
    let port = std::env::var("PORT".to_string()).unwrap();
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on http://{}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

// Application state
#[derive(Clone)]
struct AppState {
    db_pool: sqlx::PgPool,
    redis_cache: RedisCache,
    stellar_client: StellarClient,
}

// Handlers
async fn root() -> &'static str {
    "Welcome to Aframp Backend API"
}

async fn health() -> &'static str {
    "OK"
}

async fn get_stellar_account(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> Result<String, (axum::http::StatusCode, String)> {
    match state.stellar_client.account_exists(&address).await {
        Ok(exists) => {
            if exists {
                match state.stellar_client.get_account(&address).await {
                    Ok(account) => Ok(format!(
                        "Account: {}, Balances: {}",
                        account.account_id,
                        account.balances.len()
                    )),
                    Err(e) => Err((
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to fetch account: {}", e),
                    )),
                }
            } else {
                Err((
                    axum::http::StatusCode::NOT_FOUND,
                    "Account not found".to_string(),
                ))
            }
        }
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error checking account: {}", e),
        )),
    }
}
