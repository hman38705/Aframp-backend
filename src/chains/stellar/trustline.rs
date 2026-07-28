//! Compatibility shim for the historical `chains::stellar::trustline::CngnTrustlineManager`.
//!
//! Call sites across the onramp flow (`api/onramp/*`, `workers/onramp_processor.rs`,
//! `services/onramp_quote.rs`, `api/mint/validator.rs`) only ever use
//! `CngnTrustlineManager::new(stellar_client)` and `.check_trustline(account_id)`,
//! reading `.has_trustline` / `.is_authorized` off the result. This delegates to
//! [`crate::services::cngn_trustline::CngnTrustlineService`] — the actively
//! maintained implementation already built against the current
//! [`super::client::StellarClient`] — translating its `TrustlineStatus.exists`
//! field to the historical `has_trustline` name these call sites expect.
use super::client::StellarClient;
use crate::error::AppError;
use crate::services::cngn_trustline::CngnTrustlineService;

/// Trustline check result, field-compatible with the pre-migration
/// `CngnTrustlineManager::check_trustline` return shape.
#[derive(Debug, Clone)]
pub struct TrustlineCheckResult {
    pub has_trustline: bool,
    pub is_authorized: bool,
    pub balance: Option<String>,
    pub limit: Option<String>,
}

pub struct CngnTrustlineManager {
    inner: CngnTrustlineService,
}

impl CngnTrustlineManager {
    pub fn new(stellar_client: StellarClient) -> Self {
        Self {
            inner: CngnTrustlineService::new(stellar_client),
        }
    }

    pub async fn check_trustline(&self, account_id: &str) -> Result<TrustlineCheckResult, AppError> {
        let status = self.inner.check_trustline(account_id).await?;
        Ok(TrustlineCheckResult {
            has_trustline: status.exists,
            is_authorized: status.is_authorized,
            balance: status.balance,
            limit: status.limit,
        })
    }
}
