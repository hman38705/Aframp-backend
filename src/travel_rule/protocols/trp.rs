/// TRP (Travel Rule Protocol) adapter — transport skeleton.
/// Not certified for production; requires enrollment with the TRUST & Track
/// or interVASP TRP working group and signed message verification.
use crate::travel_rule::models::{EncryptedPayload, TransmissionResult, TravelRuleProtocol};
use crate::travel_rule::protocols::protocol_trait::TravelRuleProtocolAdapter;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

pub struct TrpAdapter {
    http: Client,
    our_vasp_id: String,
}

impl TrpAdapter {
    pub fn new(our_vasp_id: String) -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            our_vasp_id,
        }
    }
}

#[async_trait]
impl TravelRuleProtocolAdapter for TrpAdapter {
    fn protocol_name(&self) -> TravelRuleProtocol {
        TravelRuleProtocol::Trp
    }

    async fn send_originator_info(
        &self,
        endpoint: &str,
        exchange_id: Uuid,
        payload: &EncryptedPayload,
    ) -> Result<TransmissionResult> {
        let body = json!({
            "LEI_version": "1.0",
            "exchange_id": exchange_id.to_string(),
            "sender_vasp_id": self.our_vasp_id,
            "transfer": {
                "encrypted_payload": payload,
            }
        });

        let resp = self
            .http
            .post(format!("{}/trp/transfer", endpoint))
            .header("Content-Type", "application/trp+json")
            .header("X-TRP-Version", "2022-12")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 202 => {
                info!(exchange_id = %exchange_id, protocol = "trp", "Originator info transmitted");
                Ok(TransmissionResult {
                    success: true,
                    protocol: TravelRuleProtocol::Trp,
                    error: None,
                    acknowledged_at: Some(Utc::now()),
                })
            }
            Ok(r) => {
                let status = r.status();
                warn!(exchange_id = %exchange_id, status = %status, "TRP transmission rejected");
                Ok(TransmissionResult {
                    success: false,
                    protocol: TravelRuleProtocol::Trp,
                    error: Some(format!("HTTP {}", status)),
                    acknowledged_at: None,
                })
            }
            Err(e) => {
                warn!(exchange_id = %exchange_id, error = %e, "TRP connection failed");
                Ok(TransmissionResult {
                    success: false,
                    protocol: TravelRuleProtocol::Trp,
                    error: Some(e.to_string()),
                    acknowledged_at: None,
                })
            }
        }
    }

    async fn acknowledge_receipt(&self, endpoint: &str, exchange_id: Uuid) -> Result<()> {
        let body = json!({
            "exchange_id": exchange_id.to_string(),
            "status": "RECEIVED",
        });

        let _ = self
            .http
            .post(format!("{}/trp/acknowledge", endpoint))
            .header("Content-Type", "application/trp+json")
            .json(&body)
            .send()
            .await;

        Ok(())
    }
}
