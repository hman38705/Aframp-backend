/// Travel Rule Compliance (Issue #452)
///
/// FATF Recommendation 16 — VASP-to-VASP PII exchange at threshold.
///
/// Key behaviours:
/// - Outbound transfers above threshold enter pending_travel_rule state
/// - Protocol router: TRISA → TRP → OpenVASP, with fallback chain
/// - Encrypted off-chain PII exchange (IVMS101 schema, AES-256-GCM)
/// - Inbound transfers screened against sanctions before clearing
/// - Unhosted wallets routed per configurable policy (allow/block/attest)
/// - Sunrise rule applied when VASP exists but has no supported protocol
/// - All exchanges persisted for audit trail and regulatory reporting
///
/// Protocol adapters are transport skeletons — not certified for production
/// inter-VASP interop. Full TRISA/OpenVASP/TRP certification requires VASP
/// registration with the respective governing body and mutual TLS setup.
pub mod handlers;
pub mod metrics;
pub mod models;
pub mod protocols;
pub mod repository;
pub mod routes;
pub mod service;
pub mod worker;

#[cfg(test)]
pub mod tests;

pub use handlers::{TravelRuleState};
pub use models::*;
pub use repository::TravelRuleRepository;
pub use routes::travel_rule_router;
pub use service::TravelRuleService;
pub use worker::TravelRuleSlaWorker;
