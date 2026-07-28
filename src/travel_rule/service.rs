use crate::aml::case_management::AmlCaseManager;
use crate::aml::models::{AmlScreeningRequest, AmlScreeningResult};
use crate::aml::screening::SanctionsScreeningService;
use crate::event_bus::bus::EventBus;
use crate::event_bus::models::{EventType, PlatformEvent};
use crate::services::exchange_rate::ExchangeRateService;
use crate::travel_rule::metrics;
use crate::travel_rule::models::*;
use crate::travel_rule::protocols::ProtocolRouter;
use crate::travel_rule::repository::TravelRuleRepository;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bigdecimal::BigDecimal;
use chrono::Utc;
use rand::RngCore;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

const DEFAULT_SLA_WINDOW_SECS: i32 = 300;

pub struct TravelRuleService {
    repo: Arc<TravelRuleRepository>,
    protocol_router: Arc<ProtocolRouter>,
    sanctions: Arc<SanctionsScreeningService>,
    aml_case_manager: Arc<AmlCaseManager>,
    event_bus: Arc<EventBus>,
    exchange_rate_svc: Arc<ExchangeRateService>,
    our_vasp_id: String,
    platform_aes_key: [u8; 32],
}

impl TravelRuleService {
    pub fn new(
        repo: Arc<TravelRuleRepository>,
        sanctions: Arc<SanctionsScreeningService>,
        aml_case_manager: Arc<AmlCaseManager>,
        event_bus: Arc<EventBus>,
        exchange_rate_svc: Arc<ExchangeRateService>,
        our_vasp_id: String,
    ) -> Self {
        let key_b64 = std::env::var("TRAVEL_RULE_AES_KEY_B64").unwrap_or_default();
        let platform_aes_key = if key_b64.is_empty() {
            [0u8; 32] // dev/test only — zero key
        } else {
            let decoded = BASE64.decode(&key_b64).unwrap_or_default();
            let mut k = [0u8; 32];
            let len = decoded.len().min(32);
            k[..len].copy_from_slice(&decoded[..len]);
            k
        };

        let protocol_router = Arc::new(ProtocolRouter::new(our_vasp_id.clone()));

        Self {
            repo,
            protocol_router,
            sanctions,
            aml_case_manager,
            event_bus,
            exchange_rate_svc,
            our_vasp_id,
            platform_aes_key,
        }
    }

    // -------------------------------------------------------------------------
    // Threshold checking — converts amount to NGN via ExchangeRateService
    // -------------------------------------------------------------------------

    /// Returns `true` when this transaction must carry Travel Rule information.
    /// `amount` is in `currency`; we convert to NGN for comparison using the
    /// live ExchangeRateService (falls back to raw amount if rate unavailable).
    pub async fn requires_travel_rule(
        &self,
        amount: Decimal,
        currency: &str,
        transaction_type: &str,
        jurisdiction: &str,
    ) -> bool {
        // Cross-border below-threshold transactions still require TR (FATF threshold = 0)
        // so check that first before doing any conversion.
        if let Ok(Some(cb)) = self.repo.get_threshold(currency, "cross_border", jurisdiction).await {
            if cb.threshold_amount == Decimal::ZERO {
                return true;
            }
        }

        // Convert amount to NGN equivalent for threshold comparison
        let amount_ngn = self.to_ngn_equivalent(amount, currency).await;

        // Try jurisdiction-specific threshold, then fall back to "NG"
        if let Ok(Some(t)) = self.repo.get_threshold(currency, transaction_type, jurisdiction).await {
            return amount_ngn >= t.threshold_amount;
        }
        if let Ok(Some(t)) = self.repo.get_threshold(currency, transaction_type, "NG").await {
            return amount_ngn >= t.threshold_amount;
        }

        // No threshold configured — default to requiring Travel Rule (safe default)
        warn!(
            currency, transaction_type, jurisdiction,
            "No Travel Rule threshold configured — defaulting to require"
        );
        true
    }

    /// Convert an amount in `from_currency` to NGN using ExchangeRateService.
    /// Falls back to the raw amount on any error.
    async fn to_ngn_equivalent(&self, amount: Decimal, from_currency: &str) -> Decimal {
        if from_currency == "NGN" || from_currency == "cNGN" {
            return amount;
        }
        match self.exchange_rate_svc.get_rate(from_currency, "NGN").await {
            Ok(rate) => {
                let rate_dec = Decimal::from_str(&rate.to_string()).unwrap_or(Decimal::ONE);
                amount * rate_dec
            }
            Err(e) => {
                warn!(
                    error = %e,
                    from_currency,
                    "ExchangeRateService unavailable for NGN conversion — using raw amount"
                );
                amount
            }
        }
    }

    // -------------------------------------------------------------------------
    // Corridor high-risk override
    // -------------------------------------------------------------------------

    /// Returns `true` for FATF grey-list / high-risk jurisdictions.
    /// Travel Rule applies regardless of amount when this returns true.
    pub fn is_high_risk_corridor(&self, _origin: &str, destination: &str) -> bool {
        const HIGH_RISK: &[&str] = &[
            "IR", "KP", "MM", "BY", "CF", "CD", "HT", "IQ", "LY", "ML", "NI",
            "PK", "RU", "SO", "SS", "SY", "TZ", "UG", "VE", "YE",
        ];
        HIGH_RISK.contains(&destination)
    }

    // -------------------------------------------------------------------------
    // Outbound flow
    // -------------------------------------------------------------------------

    pub async fn initiate_outbound(
        &self,
        req: InitiateTravelRuleRequest,
    ) -> Result<TravelRuleExchange> {
        // Hard gate: reject before any exchange record is created (and before the
        // caller is allowed to proceed to disbursement) when originator/beneficiary
        // identification is incomplete. See `validate_minimum_fields` for why this
        // is a lighter bar than `validate_completeness` (used for inbound VASP-to-VASP
        // exchanges, where full IVMS101 government-ID data is expected).
        Self::validate_minimum_fields(&req.originator)?;
        Self::validate_minimum_fields(&req.beneficiary)?;

        // Discover counterparty VASP
        let destination_vasp = if let Some(addr) = &req.destination_address {
            self.repo.discover_vasp_by_wallet(addr).await.unwrap_or(None)
        } else {
            self.repo.get_vasp(&req.beneficiary_vasp_id).await.unwrap_or(None)
        };

        let (vasp_id, should_send) = match &destination_vasp {
            Some(v) => {
                if v.trust_status == VaspTrustStatus::Blocked {
                    return Err(anyhow!("Destination VASP is blocked: {}", v.vasp_id));
                }
                (v.vasp_id.clone(), true)
            }
            None => {
                // Unhosted wallet — apply policy check
                self.apply_unhosted_policy_check(
                    req.destination_address.as_deref().unwrap_or("unknown"),
                    &req.transaction_id,
                )
                .await?;
                ("unhosted".to_string(), false)
            }
        };

        let originator_json = serde_json::to_value(&req.originator)?;
        let beneficiary_json = serde_json::to_value(&req.beneficiary)?;

        let exchange = self
            .repo
            .create_exchange(
                &self.our_vasp_id,
                &vasp_id,
                &req.transaction_id,
                &TravelRuleProtocol::Unknown,
                &TravelRuleDirection::Outbound,
                originator_json.clone(),
                beneficiary_json.clone(),
                &req.transfer_amount,
                &req.asset_code,
                DEFAULT_SLA_WINDOW_SECS,
            )
            .await?;

        metrics::OUTBOUND_MESSAGES_TOTAL
            .with_label_values(&["pending"])
            .inc();

        let _ = self
            .event_bus
            .publish(PlatformEvent::new(
                EventType::TravelRuleTriggered,
                exchange.exchange_id.to_string(),
                "travel_rule_exchange",
                serde_json::json!({
                    "transaction_id": req.transaction_id,
                    "vasp_id": vasp_id,
                    "direction": "outbound",
                }),
            ))
            .await;

        if !should_send {
            info!(
                exchange_id = %exchange.exchange_id,
                "Unhosted wallet: Travel Rule data not sent, enhanced monitoring applied"
            );
            metrics::UNHOSTED_WALLET_TRANSACTIONS_TOTAL
                .with_label_values(&["allowed_unhosted"])
                .inc();
            return Ok(exchange);
        }

        let vasp = destination_vasp.unwrap();

        let has_protocol = vasp.supported_protocols.iter().any(|p| {
            matches!(p.as_str(), "trisa" | "trp" | "trust" | "open_vasp" | "openvasp")
        });

        if !has_protocol {
            self.repo.mark_sunrise_rule_applied(exchange.exchange_id).await?;
            warn!(
                exchange_id = %exchange.exchange_id,
                vasp_id = %vasp.vasp_id,
                "Sunrise rule applied — VASP has no recognized Travel Rule protocol"
            );
            metrics::SUNRISE_RULE_APPLIED_TOTAL.inc();
            return self.repo.get_exchange(exchange.exchange_id).await?
                .ok_or_else(|| anyhow!("Exchange not found after sunrise rule"));
        }

        let pii_bytes = serde_json::to_vec(&originator_json)?;
        let encrypted = self.encrypt_pii(&pii_bytes, &vasp)?;

        let tx_result = self
            .protocol_router
            .select_and_send(&vasp, exchange.exchange_id, &encrypted)
            .await;

        match tx_result {
            Some(result) if result.success => {
                self.repo.mark_acknowledged(exchange.exchange_id).await?;
                self.repo.increment_vasp_interaction(&vasp.vasp_id).await?;

                metrics::OUTBOUND_MESSAGES_TOTAL
                    .with_label_values(&[&format!("{:?}", result.protocol)])
                    .inc();

                let _ = self
                    .event_bus
                    .publish(PlatformEvent::new(
                        EventType::TravelRuleAcknowledged,
                        exchange.exchange_id.to_string(),
                        "travel_rule_exchange",
                        serde_json::json!({
                            "protocol": format!("{:?}", result.protocol),
                            "vasp_id": vasp.vasp_id,
                        }),
                    ))
                    .await;
            }
            Some(result) => {
                self.repo.mark_failed(
                    exchange.exchange_id,
                    &result.error.unwrap_or_else(|| "transmission failed".into()),
                ).await?;
                metrics::TRANSMISSION_FAILURES_TOTAL
                    .with_label_values(&["all_protocols_failed"])
                    .inc();
            }
            None => {
                self.repo.mark_sunrise_rule_applied(exchange.exchange_id).await?;
                metrics::SUNRISE_RULE_APPLIED_TOTAL.inc();
                warn!(
                    exchange_id = %exchange.exchange_id,
                    "All protocols exhausted — sunrise rule applied"
                );
            }
        }

        self.repo.get_exchange(exchange.exchange_id).await?
            .ok_or_else(|| anyhow!("Exchange not found after initiation"))
    }

    // -------------------------------------------------------------------------
    // Inbound flow
    // -------------------------------------------------------------------------

    pub async fn handle_inbound(&self, data: InboundTravelRuleData) -> Result<TravelRuleExchange> {
        self.verify_sender_signature(&data).await?;
        self.validate_completeness(&data.originator)?;

        let originator_json = serde_json::to_value(&data.originator)?;
        let beneficiary_json = serde_json::to_value(&data.beneficiary)?;

        let exchange = self
            .repo
            .create_exchange(
                &data.originator_vasp_id,
                &self.our_vasp_id,
                &data.transaction_id,
                &data.protocol_used,
                &TravelRuleDirection::Inbound,
                originator_json,
                beneficiary_json,
                &data.transfer_amount,
                &data.asset_code,
                DEFAULT_SLA_WINDOW_SECS,
            )
            .await?;

        metrics::INBOUND_MESSAGES_TOTAL
            .with_label_values(&[&format!("{:?}", data.protocol_used)])
            .inc();

        // Sanctions screen originator
        let screen_req = self.build_screening_request(&data, exchange.exchange_id)?;
        let screen_result = self.sanctions.screen(&screen_req).await;

        if !screen_result.cleared {
            // Hold transaction + create AML compliance case via AmlCaseManager
            let wallet_address = match &data.originator {
                Ivms101Person::Natural(p) => p.account_number.clone().unwrap_or_default(),
                Ivms101Person::Legal(p) => p.account_number.clone().unwrap_or_default(),
            };

            let aml_case = self
                .aml_case_manager
                .open_case(&screen_result, &wallet_address)
                .await
                .map_err(|e| {
                    error!(error = %e, "Failed to open AML case for Travel Rule screening failure");
                    e
                })?;

            let case_id = aml_case.id;
            let screening_json = serde_json::to_value(&screen_result).unwrap_or_default();

            self.repo
                .mark_screening_failed(exchange.exchange_id, screening_json, case_id)
                .await?;

            metrics::SCREENING_FAILURES_TOTAL.inc();

            error!(
                exchange_id = %exchange.exchange_id,
                originator_vasp = %data.originator_vasp_id,
                case_id = %case_id,
                "Inbound TR originator failed sanctions screening — transaction held, AML case opened"
            );

            return self.repo.get_exchange(exchange.exchange_id).await?
                .ok_or_else(|| anyhow!("Exchange not found after screening failure"));
        }

        // All checks pass — acknowledge
        self.repo.mark_acknowledged(exchange.exchange_id).await?;

        if let Some(vasp) = self.repo.get_vasp(&data.originator_vasp_id).await.unwrap_or(None) {
            let _ = self.protocol_router
                .acknowledge_inbound(&vasp, exchange.exchange_id, &data.protocol_used)
                .await;
        }

        let _ = self
            .event_bus
            .publish(PlatformEvent::new(
                EventType::TravelRuleAcknowledged,
                exchange.exchange_id.to_string(),
                "travel_rule_exchange",
                serde_json::json!({
                    "direction": "inbound",
                    "originator_vasp": data.originator_vasp_id,
                }),
            ))
            .await;

        self.repo.get_exchange(exchange.exchange_id).await?
            .ok_or_else(|| anyhow!("Exchange not found after inbound handling"))
    }

    // -------------------------------------------------------------------------
    // Retry
    // -------------------------------------------------------------------------

    pub async fn retry_failed_message(&self, exchange_id: Uuid) -> Result<TravelRuleExchange> {
        let exchange = self.repo.get_exchange(exchange_id).await?
            .ok_or_else(|| anyhow!("Exchange not found: {}", exchange_id))?;

        if exchange.status != TravelRuleStatus::Failed
            && exchange.status != TravelRuleStatus::TimedOut
        {
            return Err(anyhow!(
                "Exchange {} is not in a retryable state: {:?}",
                exchange_id,
                exchange.status
            ));
        }

        self.repo.increment_retry(exchange_id).await?;

        let vasp = self.repo.get_vasp(&exchange.beneficiary_vasp_id).await?
            .ok_or_else(|| anyhow!("VASP not found for retry: {}", exchange.beneficiary_vasp_id))?;

        let pii_bytes = serde_json::to_vec(&exchange.originator_ivms101)?;
        let encrypted = self.encrypt_pii(&pii_bytes, &vasp)?;

        match self.protocol_router.select_and_send(&vasp, exchange_id, &encrypted).await {
            Some(r) if r.success => {
                self.repo.mark_acknowledged(exchange_id).await?;
                info!(exchange_id = %exchange_id, "Travel Rule retry succeeded");
            }
            _ => {
                self.repo.mark_failed(exchange_id, "retry failed — all protocols exhausted").await?;
                metrics::TRANSMISSION_FAILURES_TOTAL
                    .with_label_values(&["retry_failed"])
                    .inc();
            }
        }

        self.repo.get_exchange(exchange_id).await?
            .ok_or_else(|| anyhow!("Exchange not found after retry"))
    }

    // -------------------------------------------------------------------------
    // Unhosted wallet policy
    // -------------------------------------------------------------------------

    /// Enforces unhosted wallet policy. Validates pre-existing attestation when
    /// policy = require_attestation.
    async fn apply_unhosted_policy_check(
        &self,
        wallet_address: &str,
        transaction_id: &str,
    ) -> Result<()> {
        let policy = self.repo.get_active_policy().await?;

        let policy_type = policy
            .as_ref()
            .map(|p| p.policy_type.as_str())
            .unwrap_or("allow_below_threshold");

        metrics::UNHOSTED_WALLET_TRANSACTIONS_TOTAL
            .with_label_values(&[policy_type])
            .inc();

        match policy_type {
            "block" => {
                warn!(wallet = %wallet_address, "Unhosted wallet transaction blocked by policy");
                Err(anyhow!(
                    "Transaction to unhosted wallet {} is blocked by compliance policy",
                    wallet_address
                ))
            }
            "require_attestation" => {
                // Verify the user has already submitted self-attestation for this tx
                let att = self.repo.get_attestation_by_transaction(transaction_id).await?;
                if att.is_none() {
                    warn!(
                        wallet = %wallet_address,
                        transaction_id = %transaction_id,
                        "Unhosted wallet requires self-attestation — none found"
                    );
                    return Err(anyhow!(
                        "Unhosted wallet transaction requires self-attestation. \
                         Submit POST /api/compliance/travel-rule/attest first."
                    ));
                }
                info!(
                    wallet = %wallet_address,
                    transaction_id = %transaction_id,
                    "Unhosted wallet: attestation verified"
                );
                Ok(())
            }
            _ => Ok(()), // allow / allow_below_threshold
        }
    }

    pub async fn submit_attestation(
        &self,
        req: SelfAttestationRequest,
    ) -> Result<TravelRuleAttestation> {
        let attestation = self.repo.create_attestation(&req).await?;
        info!(
            user_id = %req.user_id,
            transaction_id = %req.transaction_id,
            wallet = %req.wallet_address,
            "Self-attestation recorded for unhosted wallet"
        );
        Ok(attestation)
    }

    // -------------------------------------------------------------------------
    // Encryption helpers
    // -------------------------------------------------------------------------

    pub fn encrypt_pii(&self, plaintext: &[u8], _vasp: &VaspRegistryEntry) -> Result<EncryptedPayload> {
        let mut aes_key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut aes_key_bytes);

        let key = Key::<Aes256Gcm>::from_slice(&aes_key_bytes);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow!("AES-GCM encrypt failed: {:?}", e))?;

        // Wrap AES key with platform key (XOR stub — production: RSA-OAEP per VASP public key)
        let encrypted_key = self.wrap_aes_key_with_platform_key(&aes_key_bytes)?;

        Ok(EncryptedPayload {
            ciphertext_b64: BASE64.encode(&ciphertext),
            encrypted_key_b64: BASE64.encode(&encrypted_key),
            nonce_b64: BASE64.encode(nonce_bytes),
        })
    }

    pub fn decrypt_pii(&self, payload: &EncryptedPayload) -> Result<Vec<u8>> {
        let ciphertext = BASE64.decode(&payload.ciphertext_b64)?;
        let encrypted_key = BASE64.decode(&payload.encrypted_key_b64)?;
        let nonce_bytes = BASE64.decode(&payload.nonce_b64)?;

        let aes_key_bytes = self.unwrap_aes_key(&encrypted_key)?;
        let key = Key::<Aes256Gcm>::from_slice(&aes_key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| anyhow!("AES-GCM decrypt failed: {:?}", e))
    }

    fn wrap_aes_key_with_platform_key(&self, aes_key: &[u8]) -> Result<Vec<u8>> {
        Ok(aes_key
            .iter()
            .zip(self.platform_aes_key.iter().cycle())
            .map(|(a, b)| a ^ b)
            .collect())
    }

    fn unwrap_aes_key(&self, wrapped: &[u8]) -> Result<Vec<u8>> {
        Ok(wrapped
            .iter()
            .zip(self.platform_aes_key.iter().cycle())
            .map(|(a, b)| a ^ b)
            .collect())
    }

    // -------------------------------------------------------------------------
    // IVMS101 validation
    // -------------------------------------------------------------------------

    /// Minimum field validation for outbound transfers: name + account/wallet
    /// identification. This is deliberately lighter than `validate_completeness`
    /// (DOB + national ID are not yet sourced at offramp/partner-transfer
    /// initiation — see callers) but still enforces that a transfer above the
    /// Travel Rule threshold cannot proceed with an unidentified counterparty.
    pub fn validate_minimum_fields(person: &Ivms101Person) -> Result<()> {
        match person {
            Ivms101Person::Natural(p) => {
                if p.first_name.trim().is_empty() && p.last_name.trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: originator/beneficiary name is required"));
                }
                if p.account_number.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: account_number is required"));
                }
                Ok(())
            }
            Ivms101Person::Legal(p) => {
                if p.legal_name.trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: legal_name is required"));
                }
                if p.account_number.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: account_number is required"));
                }
                Ok(())
            }
        }
    }

    pub fn validate_completeness(&self, person: &Ivms101Person) -> Result<()> {
        match person {
            Ivms101Person::Natural(p) => {
                if p.first_name.trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: first_name is required"));
                }
                if p.last_name.trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: last_name is required"));
                }
                if p.account_number.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: account_number is required"));
                }
                if p.national_id.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: national_id is required"));
                }
                if p.date_of_birth.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: date_of_birth is required"));
                }
                if p.country_of_residence.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: country_of_residence is required"));
                }
                Ok(())
            }
            Ivms101Person::Legal(p) => {
                if p.legal_name.trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: legal_name is required"));
                }
                if p.account_number.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(anyhow!("IVMS101 validation: account_number is required"));
                }
                Ok(())
            }
        }
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    async fn verify_sender_signature(&self, data: &InboundTravelRuleData) -> Result<()> {
        let vasp = self.repo.get_vasp(&data.originator_vasp_id).await?
            .ok_or_else(|| anyhow!("Unknown originator VASP: {}", data.originator_vasp_id))?;

        if vasp.trust_status == VaspTrustStatus::Blocked {
            return Err(anyhow!("Originator VASP is blocked: {}", vasp.vasp_id));
        }

        // Production: verify HMAC-SHA256 over body using vasp.public_key_pem
        // Stub: registry membership + non-blocked status is the gate
        if data.sender_signature.is_none() {
            warn!(
                vasp_id = %data.originator_vasp_id,
                "No sender signature present — accepted on registry membership"
            );
        }
        Ok(())
    }

    fn build_screening_request(
        &self,
        data: &InboundTravelRuleData,
        exchange_id: Uuid,
    ) -> Result<AmlScreeningRequest> {
        let (sender_name, sender_id, wallet_address) = match &data.originator {
            Ivms101Person::Natural(p) => (
                format!("{} {}", p.first_name, p.last_name),
                p.national_id.clone().unwrap_or_default(),
                p.account_number.clone().unwrap_or_default(),
            ),
            Ivms101Person::Legal(p) => (
                p.legal_name.clone(),
                p.registration_number.clone().unwrap_or_default(),
                p.account_number.clone().unwrap_or_default(),
            ),
        };

        let recipient_name = match &data.beneficiary {
            Ivms101Person::Natural(p) => format!("{} {}", p.first_name, p.last_name),
            Ivms101Person::Legal(p) => p.legal_name.clone(),
        };

        Ok(AmlScreeningRequest {
            transaction_id: exchange_id,
            wallet_address,
            sender_name,
            sender_id,
            recipient_name,
            recipient_id: self.our_vasp_id.clone(),
            amount: data.transfer_amount.clone(),
            from_currency: data.asset_code.clone(),
            to_currency: data.asset_code.clone(),
            origin_country: "NG".into(),
            destination_country: "NG".into(),
            created_at: Utc::now(),
        })
    }
}
