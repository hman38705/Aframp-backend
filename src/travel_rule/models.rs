use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "travel_rule_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TravelRuleStatus {
    Pending,
    Acknowledged,
    Failed,
    ManualReview,
    TimedOut,
    Completed,
}

/// Canonical protocol list per Issue #452 (OpenVASP, TRP, TRISA).
/// `Trust` kept as DB-compat alias for `Trp`; `Ivms101Direct` retained for
/// legacy rows created by the v1 stub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "travel_rule_protocol", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TravelRuleProtocol {
    Trisa,
    Trust,
    OpenVasp,
    Ivms101Direct,
    Unknown,
    Trp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "travel_rule_direction", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TravelRuleDirection {
    Outbound,
    Inbound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "vasp_trust_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VaspTrustStatus {
    Verified,
    Unverified,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "vasp_regulatory_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VaspRegulatoryStatus {
    Licensed,
    Unlicensed,
    Pending,
    Suspended,
}

// ---------------------------------------------------------------------------
// IVMS101 identity types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ivms101NaturalPerson {
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<String>,
    pub national_id: Option<String>,
    pub address: Option<String>,
    pub country_of_residence: Option<String>,
    /// Originator wallet address or account number
    pub account_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ivms101LegalPerson {
    pub legal_name: String,
    pub registration_number: Option<String>,
    pub country_of_registration: Option<String>,
    pub address: Option<String>,
    pub account_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ivms101Person {
    Natural(Ivms101NaturalPerson),
    Legal(Ivms101LegalPerson),
}

// ---------------------------------------------------------------------------
// Core DB models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TravelRuleExchange {
    pub exchange_id: Uuid,
    pub transaction_id: String,
    pub originator_vasp_id: String,
    pub beneficiary_vasp_id: String,
    pub protocol_used: TravelRuleProtocol,
    pub status: TravelRuleStatus,
    pub originator_ivms101: Value,
    pub beneficiary_ivms101: Value,
    pub transfer_amount: String,
    pub asset_code: String,
    pub handshake_initiated_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub timeout_at: DateTime<Utc>,
    pub failure_reason: Option<String>,
    // v2 fields
    pub direction: TravelRuleDirection,
    pub sunrise_rule_applied: bool,
    pub sla_window_secs: i32,
    pub sla_breached: bool,
    pub sla_breached_at: Option<DateTime<Utc>>,
    pub screening_result: Option<Value>,
    pub compliance_case_id: Option<Uuid>,
    pub retry_count: i32,
    pub last_retry_at: Option<DateTime<Utc>>,
    pub pending_travel_rule: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VaspRegistryEntry {
    pub vasp_id: String,
    pub vasp_name: String,
    pub supported_protocols: Vec<String>,
    pub travel_rule_endpoint: Option<String>,
    pub is_verified: bool,
    pub jurisdiction: String,
    pub last_verified_at: Option<DateTime<Utc>>,
    // v2 fields
    pub vasp_did: Option<String>,
    pub lei: Option<String>,
    pub regulatory_status: VaspRegulatoryStatus,
    pub trust_status: VaspTrustStatus,
    pub public_key_pem: Option<String>,
    pub interaction_count: i32,
    pub last_interaction_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TravelRuleThreshold {
    pub id: Uuid,
    pub currency: String,
    pub transaction_type: String,
    pub jurisdiction: String,
    pub threshold_amount: Decimal,
    pub is_active: bool,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UnhostedWalletPolicy {
    pub id: Uuid,
    pub policy_type: String,
    pub threshold_amount: Option<Decimal>,
    pub threshold_currency: Option<String>,
    pub updated_by_compliance_officer: Option<Uuid>,
    pub updated_by_senior_management: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TravelRuleAttestation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub transaction_id: String,
    pub wallet_address: String,
    pub attested_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateTravelRuleRequest {
    pub transaction_id: String,
    pub beneficiary_vasp_id: String,
    pub originator: Ivms101Person,
    pub beneficiary: Ivms101Person,
    pub transfer_amount: String,
    pub asset_code: String,
    /// Destination wallet address for VASP discovery
    pub destination_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundTravelRuleData {
    pub exchange_id: Uuid,
    pub originator_vasp_id: String,
    pub transaction_id: String,
    pub originator: Ivms101Person,
    pub beneficiary: Ivms101Person,
    pub transfer_amount: String,
    pub asset_code: String,
    pub protocol_used: TravelRuleProtocol,
    /// Base64-encoded HMAC-SHA256 signature over the raw JSON body
    pub sender_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterVaspRequest {
    pub vasp_id: String,
    pub vasp_name: String,
    pub jurisdiction: String,
    pub supported_protocols: Vec<String>,
    pub travel_rule_endpoint: Option<String>,
    pub vasp_did: Option<String>,
    pub lei: Option<String>,
    pub regulatory_status: Option<VaspRegulatoryStatus>,
    pub trust_status: Option<VaspTrustStatus>,
    pub public_key_pem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateVaspRequest {
    pub vasp_name: Option<String>,
    pub supported_protocols: Option<Vec<String>>,
    pub travel_rule_endpoint: Option<String>,
    pub vasp_did: Option<String>,
    pub lei: Option<String>,
    pub regulatory_status: Option<VaspRegulatoryStatus>,
    pub trust_status: Option<VaspTrustStatus>,
    pub public_key_pem: Option<String>,
    pub is_verified: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListVaspsQuery {
    pub jurisdiction: Option<String>,
    pub trust_status: Option<String>,
    pub protocol: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TravelRuleMessageListQuery {
    pub direction: Option<String>,
    pub status: Option<String>,
    pub vasp_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateThresholdsRequest {
    pub thresholds: Vec<ThresholdUpdateItem>,
    /// Compliance officer user ID (required)
    pub approved_by: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdUpdateItem {
    pub currency: String,
    pub transaction_type: String,
    pub jurisdiction: String,
    pub threshold_amount: Decimal,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUnhostedWalletPolicyRequest {
    pub policy_type: String,
    pub threshold_amount: Option<Decimal>,
    pub threshold_currency: Option<String>,
    /// Compliance officer approval — required
    pub compliance_officer_id: Uuid,
    /// Senior management approval — required for policy changes
    pub senior_management_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfAttestationRequest {
    pub user_id: Uuid,
    pub transaction_id: String,
    pub wallet_address: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelRuleMetricsQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelRuleMetrics {
    pub period_from: DateTime<Utc>,
    pub period_to: DateTime<Utc>,
    pub total_threshold_triggering: i64,
    pub successful_exchanges: i64,
    pub failed_exchanges: i64,
    pub unhosted_wallet_transactions: i64,
    pub sunrise_rule_applications: i64,
    pub inbound_received: i64,
    pub inbound_screening_failures: i64,
    pub outbound_by_protocol: std::collections::HashMap<String, i64>,
    pub vasp_registry_size: i64,
    pub unacknowledged_outbound: i64,
    pub pending_inbound_screening: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// AES-GCM ciphertext, base64-encoded
    pub ciphertext_b64: String,
    /// AES key encrypted with recipient's RSA public key, base64-encoded
    pub encrypted_key_b64: String,
    /// GCM nonce, base64-encoded
    pub nonce_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmissionResult {
    pub success: bool,
    pub protocol: TravelRuleProtocol,
    pub error: Option<String>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}
