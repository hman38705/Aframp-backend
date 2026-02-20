//! Services module for business logic and integrations

pub mod balance;
#[cfg(feature = "database")]
pub mod cngn_trustline;
#[cfg(feature = "database")]
pub mod cngn_payment_builder;
#[cfg(feature = "database")]
pub mod conversion_audit;
#[cfg(feature = "database")]
pub mod fee_structure;
#[cfg(feature = "database")]
pub mod trustline_operation;
#[cfg(feature = "database")]
pub mod payment_orchestrator;

// Re-export blockchain traits for convenience
#[cfg(feature = "database")]
pub use crate::chains::traits::{
    AggregatedBalance, BlockchainError, BlockchainResult, BlockchainService, ChainHealthStatus,
    ChainType, FeeEstimate, MultiChainBalanceAggregator, TotalBalance, TransactionBuilder,
    TransactionHandler, TransactionResult, TxParams,
};

// Re-export orchestrator types
#[cfg(feature = "database")]
pub use crate::services::payment_orchestrator::{
    OrchestratorConfig, OrchestratorError, OrchestratorResult, OrchestrationState,
    PaymentInitiationRequest, PaymentOrchestrator, ProviderHealth, ProviderMetrics,
    SelectionContext, SelectionStrategy,
};
