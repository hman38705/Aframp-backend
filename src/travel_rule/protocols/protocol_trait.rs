use crate::travel_rule::models::{EncryptedPayload, TransmissionResult, TravelRuleProtocol};
use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// Implemented by every Travel Rule protocol adapter.
/// Transport skeleton — not certified for production inter-VASP interop;
/// full TRISA/OpenVASP/TRP certification requires VASP registration and
/// mutual TLS certificate exchange with the respective governing body.
#[async_trait]
pub trait TravelRuleProtocolAdapter: Send + Sync {
    fn protocol_name(&self) -> TravelRuleProtocol;

    /// Transmit encrypted originator information to the counterparty VASP.
    async fn send_originator_info(
        &self,
        endpoint: &str,
        exchange_id: Uuid,
        payload: &EncryptedPayload,
    ) -> Result<TransmissionResult>;

    /// Acknowledge receipt of an inbound message.
    async fn acknowledge_receipt(&self, endpoint: &str, exchange_id: Uuid) -> Result<()>;
}
