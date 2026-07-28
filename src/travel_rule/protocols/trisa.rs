/// TRISA (Travel Rule Information Sharing Architecture) adapter — transport skeleton.
/// Not certified for production; requires TRISA Certificate Authority registration
/// and mutual TLS with the TRISA Global Directory Service.
use crate::travel_rule::models::{EncryptedPayload, TransmissionResult, TravelRuleProtocol};
use crate::travel_rule::protocols::protocol_trait::TravelRuleProtocolAdapter;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

pub struct TrisaAdapter {
    http: Client,
    our_vasp_id: String,
}

impl TrisaAdapter {
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
impl TravelRuleProtocolAdapter for TrisaAdapter {
    fn protocol_name(&self) -> TravelRuleProtocol {
        TravelRuleProtocol::Trisa
    }

    async fn send_originator_info(
        &self,
        endpoint: &str,
        exchange_id: Uuid,
        payload: &EncryptedPayload,
    ) -> Result<TransmissionResult> {
        let body = json!({
            "identity": {
                "version": 1,
                "exchange_id": exchange_id.to_string(),
                "originator_vasp": self.our_vasp_id,
                "encrypted_payload": payload,
            }
        });

        let resp = self
            .http
            .post(format!("{}/keyexchange", endpoint))
            .header("Content-Type", "application/json")
            .header("X-TRISA-Version", "v1beta1")
            .header("X-TRISA-VASP-ID", &self.our_vasp_id)
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 202 => {
                info!(exchange_id = %exchange_id, protocol = "trisa", "Originator info transmitted");
                Ok(TransmissionResult {
                    success: true,
                    protocol: TravelRuleProtocol::Trisa,
                    error: None,
                    acknowledged_at: Some(Utc::now()),
                })
            }
            Ok(r) => {
                let status = r.status();
                warn!(exchange_id = %exchange_id, status = %status, "TRISA transmission rejected");
                Ok(TransmissionResult {
                    success: false,
                    protocol: TravelRuleProtocol::Trisa,
                    error: Some(format!("HTTP {}", status)),
                    acknowledged_at: None,
                })
            }
            Err(e) => {
                warn!(exchange_id = %exchange_id, error = %e, "TRISA connection failed");
                Ok(TransmissionResult {
                    success: false,
                    protocol: TravelRuleProtocol::Trisa,
                    error: Some(e.to_string()),
                    acknowledged_at: None,
                })
            }
        }
    }

    async fn acknowledge_receipt(&self, endpoint: &str, exchange_id: Uuid) -> Result<()> {
        let body = json!({
            "exchange_id": exchange_id.to_string(),
            "acknowledged": true,
        });

        let _ = self
            .http
            .post(format!("{}/acknowledge", endpoint))
            .header("X-TRISA-Version", "v1beta1")
            .json(&body)
            .send()
            .await;

        Ok(())
    }
}
