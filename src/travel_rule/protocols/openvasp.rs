/// OpenVASP protocol adapter — transport skeleton.
/// Not certified for production inter-VASP interop; requires VASP registration
/// with the OpenVASP Association and symmetric session keys.
use crate::travel_rule::models::{EncryptedPayload, TransmissionResult, TravelRuleProtocol};
use crate::travel_rule::protocols::protocol_trait::TravelRuleProtocolAdapter;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

pub struct OpenVaspAdapter {
    http: Client,
    our_vasp_id: String,
}

impl OpenVaspAdapter {
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
impl TravelRuleProtocolAdapter for OpenVaspAdapter {
    fn protocol_name(&self) -> TravelRuleProtocol {
        TravelRuleProtocol::OpenVasp
    }

    async fn send_originator_info(
        &self,
        endpoint: &str,
        exchange_id: Uuid,
        payload: &EncryptedPayload,
        ) -> Result<TransmissionResult> {
        let body = json!({
            "version": "1.0",
            "type": "TRANSFER_REQUEST",
            "originator_vasp_id": self.our_vasp_id,
            "exchange_id": exchange_id.to_string(),
            "encrypted_payload": payload,
        });

        let resp = self
            .http
            .post(format!("{}/transfer", endpoint))
            .header("Content-Type", "application/json")
            .header("X-OpenVASP-Version", "1.0")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 202 => {
                info!(exchange_id = %exchange_id, protocol = "openvasp", "Originator info transmitted");
                Ok(TransmissionResult {
                    success: true,
                    protocol: TravelRuleProtocol::OpenVasp,
                    error: None,
                    acknowledged_at: Some(Utc::now()),
                })
            }
            Ok(r) => {
                let status = r.status();
                warn!(exchange_id = %exchange_id, status = %status, "OpenVASP transmission rejected");
                Ok(TransmissionResult {
                    success: false,
                    protocol: TravelRuleProtocol::OpenVasp,
                    error: Some(format!("HTTP {}", status)),
                    acknowledged_at: None,
                })
            }
            Err(e) => {
                warn!(exchange_id = %exchange_id, error = %e, "OpenVASP connection failed");
                Ok(TransmissionResult {
                    success: false,
                    protocol: TravelRuleProtocol::OpenVasp,
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
            .post(format!("{}/acknowledge", endpoint))
            .header("X-OpenVASP-Version", "1.0")
            .json(&body)
            .send()
            .await;

        Ok(())
    }
}
