use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct DetectedDeposit {
    pub tx_hash: String,
    pub destination: String,
    pub amount_stroops: i64,
    pub asset: String,
    pub confirmations: i32,
}

#[async_trait]
pub trait BlockchainListener: Send + Sync {
    async fn fetch_deposits(&self) -> Result<Vec<DetectedDeposit>, String>;
}

pub struct StellarListener {
    pub horizon_url: String,
    pub system_wallet: String,
}

#[async_trait]
impl BlockchainListener for StellarListener {
    async fn fetch_deposits(&self) -> Result<Vec<DetectedDeposit>, String> {
        // TODO: query Horizon for payments to `system_wallet`, map to DetectedDeposit.
        let _ = (&self.horizon_url, &self.system_wallet);
        Ok(vec![])
    }
}