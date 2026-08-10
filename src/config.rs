use std::sync::Arc;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub bind_addr: String,
    pub jwt_secret: Arc<String>,
    pub webhook_secret: Arc<String>,
    pub stellar_system_wallet: Arc<String>,
    pub stellar_horizon_url: String,
    pub stellar_poll_interval_secs: u64,
    pub wallet_encryption_key: Arc<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: env("DATABASE_URL")?,
            bind_addr: std::env::var("APP_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into()),
            jwt_secret: Arc::new(env("JWT_SECRET")?),
            webhook_secret: Arc::new(env("WEBHOOK_SECRET")?),
            stellar_system_wallet: Arc::new(env("STELLAR_SYSTEM_WALLET_ADDRESS")?),
            stellar_horizon_url: std::env::var("STELLAR_HORIZON_URL")
                .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".into()),
            stellar_poll_interval_secs: std::env::var("STELLAR_POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            wallet_encryption_key: Arc::new(env("WALLET_ENCRYPTION_KEY")?),
        })
    }
}

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is required"))
}
