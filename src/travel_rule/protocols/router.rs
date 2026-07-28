use crate::travel_rule::models::{
    EncryptedPayload, TransmissionResult, TravelRuleProtocol, VaspRegistryEntry,
};
use crate::travel_rule::protocols::openvasp::OpenVaspAdapter;
use crate::travel_rule::protocols::protocol_trait::TravelRuleProtocolAdapter;
use crate::travel_rule::protocols::trisa::TrisaAdapter;
use crate::travel_rule::protocols::trp::TrpAdapter;
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct ProtocolRouter {
    trisa: Arc<TrisaAdapter>,
    trp: Arc<TrpAdapter>,
    openvasp: Arc<OpenVaspAdapter>,
}

impl ProtocolRouter {
    pub fn new(our_vasp_id: String) -> Self {
        Self {
            trisa: Arc::new(TrisaAdapter::new(our_vasp_id.clone())),
            trp: Arc::new(TrpAdapter::new(our_vasp_id.clone())),
            openvasp: Arc::new(OpenVaspAdapter::new(our_vasp_id)),
        }
    }

    /// Attempt transmission in priority order: TRISA → TRP → OpenVASP.
    /// Falls back to next protocol if the current one fails.
    /// Returns `None` if all protocols fail (caller applies sunrise rule).
    pub async fn select_and_send(
        &self,
        vasp: &VaspRegistryEntry,
        exchange_id: Uuid,
        payload: &EncryptedPayload,
    ) -> Option<TransmissionResult> {
        let endpoint = match &vasp.travel_rule_endpoint {
            Some(ep) => ep.clone(),
            None => {
                warn!(
                    vasp_id = %vasp.vasp_id,
                    "No travel rule endpoint for VASP — cannot transmit"
                );
                return None;
            }
        };

        let adapters = self.build_preference_chain(&vasp.supported_protocols);

        for adapter in &adapters {
            let result = adapter
                .send_originator_info(&endpoint, exchange_id, payload)
                .await;

            match result {
                Ok(r) if r.success => {
                    info!(
                        exchange_id = %exchange_id,
                        protocol = ?r.protocol,
                        "Travel Rule transmission succeeded"
                    );
                    return Some(r);
                }
                Ok(r) => {
                    warn!(
                        exchange_id = %exchange_id,
                        protocol = ?r.protocol,
                        error = ?r.error,
                        "Protocol attempt failed, trying next"
                    );
                }
                Err(e) => {
                    warn!(
                        exchange_id = %exchange_id,
                        error = %e,
                        "Protocol adapter error, trying next"
                    );
                }
            }
        }

        warn!(
            exchange_id = %exchange_id,
            vasp_id = %vasp.vasp_id,
            "All Travel Rule protocols exhausted"
        );
        None
    }

    /// Acknowledge receipt of an inbound message using the preferred protocol.
    pub async fn acknowledge_inbound(
        &self,
        vasp: &VaspRegistryEntry,
        exchange_id: Uuid,
        protocol_used: &TravelRuleProtocol,
    ) -> Result<()> {
        let endpoint = match &vasp.travel_rule_endpoint {
            Some(ep) => ep.clone(),
            None => return Ok(()), // best-effort
        };

        let adapter: &dyn TravelRuleProtocolAdapter = match protocol_used {
            TravelRuleProtocol::Trisa => self.trisa.as_ref(),
            TravelRuleProtocol::Trp | TravelRuleProtocol::Trust => self.trp.as_ref(),
            TravelRuleProtocol::OpenVasp => self.openvasp.as_ref(),
            _ => self.trisa.as_ref(),
        };

        adapter.acknowledge_receipt(&endpoint, exchange_id).await
    }

    fn build_preference_chain(
        &self,
        supported: &[String],
    ) -> Vec<&dyn TravelRuleProtocolAdapter> {
        let mut chain: Vec<&dyn TravelRuleProtocolAdapter> = Vec::new();

        // Priority: TRISA first (most adopted globally), then TRP, then OpenVASP
        if supported.iter().any(|p| p == "trisa") {
            chain.push(self.trisa.as_ref());
        }
        if supported.iter().any(|p| p == "trp" || p == "trust") {
            chain.push(self.trp.as_ref());
        }
        if supported.iter().any(|p| p == "open_vasp" || p == "openvasp") {
            chain.push(self.openvasp.as_ref());
        }

        // If VASP has no recognized protocol, try all in preference order
        if chain.is_empty() {
            chain.push(self.trisa.as_ref());
            chain.push(self.trp.as_ref());
            chain.push(self.openvasp.as_ref());
        }

        chain
    }
}
