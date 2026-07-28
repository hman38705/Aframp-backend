use crate::travel_rule::models::*;
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

pub struct TravelRuleRepository {
    pool: PgPool,
}

impl TravelRuleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -------------------------------------------------------------------------
    // VASP Registry
    // -------------------------------------------------------------------------

    pub async fn create_vasp(&self, req: &RegisterVaspRequest) -> Result<VaspRegistryEntry> {
        let regulatory_status = req
            .regulatory_status
            .clone()
            .unwrap_or(VaspRegulatoryStatus::Unlicensed);
        let trust_status = req
            .trust_status
            .clone()
            .unwrap_or(VaspTrustStatus::Unverified);
        let now = Utc::now();

        let entry = sqlx::query_as::<_, VaspRegistryEntry>(
            r#"INSERT INTO vasp_registry (
                vasp_id, vasp_name, jurisdiction, supported_protocols, travel_rule_endpoint,
                vasp_did, lei, regulatory_status, trust_status, public_key_pem,
                is_verified, interaction_count, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,false,0,$11,$12)
            RETURNING *"#,
        )
        .bind(&req.vasp_id)
        .bind(&req.vasp_name)
        .bind(&req.jurisdiction)
        .bind(&req.supported_protocols)
        .bind(&req.travel_rule_endpoint)
        .bind(&req.vasp_did)
        .bind(&req.lei)
        .bind(&regulatory_status)
        .bind(&trust_status)
        .bind(&req.public_key_pem)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(entry)
    }

    pub async fn list_vasps(&self, query: &ListVaspsQuery) -> Result<Vec<VaspRegistryEntry>> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(50).min(200);
        let offset = (page - 1) * page_size;

        let entries = sqlx::query_as::<_, VaspRegistryEntry>(
            r#"SELECT * FROM vasp_registry
               WHERE ($1::TEXT IS NULL OR jurisdiction = $1)
                 AND ($2::TEXT IS NULL OR trust_status::TEXT = $2)
                 AND ($3::TEXT IS NULL OR $3 = ANY(supported_protocols))
               ORDER BY vasp_name
               LIMIT $4 OFFSET $5"#,
        )
        .bind(&query.jurisdiction)
        .bind(&query.trust_status)
        .bind(&query.protocol)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    pub async fn get_vasp(&self, vasp_id: &str) -> Result<Option<VaspRegistryEntry>> {
        let entry = sqlx::query_as::<_, VaspRegistryEntry>(
            "SELECT * FROM vasp_registry WHERE vasp_id = $1",
        )
        .bind(vasp_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }

    pub async fn update_vasp(&self, vasp_id: &str, req: &UpdateVaspRequest) -> Result<VaspRegistryEntry> {
        let existing = self
            .get_vasp(vasp_id)
            .await?
            .ok_or_else(|| anyhow!("VASP not found: {}", vasp_id))?;

        let entry = sqlx::query_as::<_, VaspRegistryEntry>(
            r#"UPDATE vasp_registry SET
                vasp_name           = COALESCE($2, vasp_name),
                supported_protocols = COALESCE($3, supported_protocols),
                travel_rule_endpoint = COALESCE($4, travel_rule_endpoint),
                vasp_did            = COALESCE($5, vasp_did),
                lei                 = COALESCE($6, lei),
                regulatory_status   = COALESCE($7, regulatory_status),
                trust_status        = COALESCE($8, trust_status),
                public_key_pem      = COALESCE($9, public_key_pem),
                is_verified         = COALESCE($10, is_verified),
                last_verified_at    = CASE WHEN $10 IS TRUE THEN NOW() ELSE last_verified_at END,
                updated_at          = NOW()
               WHERE vasp_id = $1
               RETURNING *"#,
        )
        .bind(vasp_id)
        .bind(&req.vasp_name)
        .bind(&req.supported_protocols)
        .bind(&req.travel_rule_endpoint)
        .bind(&req.vasp_did)
        .bind(&req.lei)
        .bind(&req.regulatory_status)
        .bind(&req.trust_status)
        .bind(&req.public_key_pem)
        .bind(req.is_verified)
        .fetch_one(&self.pool)
        .await?;

        info!(vasp_id = vasp_id, "VASP registry entry updated");
        let _ = existing; // used implicitly for existence check
        Ok(entry)
    }

    /// Discover a VASP from a destination wallet address prefix.
    /// Returns `None` for unhosted wallets — callers apply unhosted policy.
    pub async fn discover_vasp_by_wallet(&self, wallet_address: &str) -> Result<Option<VaspRegistryEntry>> {
        let row = sqlx::query_as::<_, VaspRegistryEntry>(
            r#"SELECT vr.*
               FROM vasp_registry vr
               JOIN vasp_wallet_labels vwl ON vr.vasp_id = vwl.vasp_id
               WHERE $1 LIKE (vwl.address_prefix || '%')
               ORDER BY length(vwl.address_prefix) DESC
               LIMIT 1"#,
        )
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await?;

        if row.is_none() {
            warn!(
                wallet = %wallet_address,
                "VASP discovery: no registry match — treating as unhosted wallet"
            );
        }

        Ok(row)
    }

    pub async fn increment_vasp_interaction(&self, vasp_id: &str) -> Result<()> {
        sqlx::query(
            r#"UPDATE vasp_registry
               SET interaction_count = interaction_count + 1,
                   last_interaction_at = NOW(),
                   updated_at = NOW()
               WHERE vasp_id = $1"#,
        )
        .bind(vasp_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn vasp_registry_size(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vasp_registry")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    // -------------------------------------------------------------------------
    // Exchanges
    // -------------------------------------------------------------------------

    pub async fn create_exchange(
        &self,
        originator_vasp_id: &str,
        beneficiary_vasp_id: &str,
        transaction_id: &str,
        protocol: &TravelRuleProtocol,
        direction: &TravelRuleDirection,
        originator_json: serde_json::Value,
        beneficiary_json: serde_json::Value,
        transfer_amount: &str,
        asset_code: &str,
        sla_window_secs: i32,
    ) -> Result<TravelRuleExchange> {
        let now = Utc::now();
        let timeout_at = now + Duration::seconds(sla_window_secs as i64);

        let exchange = sqlx::query_as::<_, TravelRuleExchange>(
            r#"INSERT INTO travel_rule_exchanges (
                exchange_id, transaction_id, originator_vasp_id, beneficiary_vasp_id,
                protocol_used, direction, status, originator_ivms101, beneficiary_ivms101,
                transfer_amount, asset_code, handshake_initiated_at, timeout_at,
                sla_window_secs, pending_travel_rule, created_at, updated_at
            ) VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, 'pending', $6, $7,
                $8, $9, $10, $11, $12, true, $10, $10
            ) RETURNING *"#,
        )
        .bind(transaction_id)
        .bind(originator_vasp_id)
        .bind(beneficiary_vasp_id)
        .bind(protocol)
        .bind(direction)
        .bind(&originator_json)
        .bind(&beneficiary_json)
        .bind(transfer_amount)
        .bind(asset_code)
        .bind(now)
        .bind(timeout_at)
        .bind(sla_window_secs)
        .fetch_one(&self.pool)
        .await?;

        Ok(exchange)
    }

    pub async fn list_exchanges(&self, query: &TravelRuleMessageListQuery) -> Result<Vec<TravelRuleExchange>> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(50).min(200);
        let offset = (page - 1) * page_size;

        let exchanges = sqlx::query_as::<_, TravelRuleExchange>(
            r#"SELECT * FROM travel_rule_exchanges
               WHERE ($1::TEXT IS NULL OR direction::TEXT = $1)
                 AND ($2::TEXT IS NULL OR status::TEXT = $2)
                 AND ($3::TEXT IS NULL OR originator_vasp_id = $3 OR beneficiary_vasp_id = $3)
                 AND ($4::TIMESTAMPTZ IS NULL OR created_at >= $4)
                 AND ($5::TIMESTAMPTZ IS NULL OR created_at <= $5)
               ORDER BY created_at DESC
               LIMIT $6 OFFSET $7"#,
        )
        .bind(&query.direction)
        .bind(&query.status)
        .bind(&query.vasp_id)
        .bind(query.from)
        .bind(query.to)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(exchanges)
    }

    pub async fn get_exchange(&self, exchange_id: Uuid) -> Result<Option<TravelRuleExchange>> {
        let ex = sqlx::query_as::<_, TravelRuleExchange>(
            "SELECT * FROM travel_rule_exchanges WHERE exchange_id = $1",
        )
        .bind(exchange_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(ex)
    }

    pub async fn mark_acknowledged(&self, exchange_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET status = 'acknowledged', acknowledged_at = NOW(),
                   pending_travel_rule = false, updated_at = NOW()
               WHERE exchange_id = $1"#,
        )
        .bind(exchange_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_failed(&self, exchange_id: Uuid, reason: &str) -> Result<()> {
        sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET status = 'failed', failure_reason = $2,
                   pending_travel_rule = false, updated_at = NOW()
               WHERE exchange_id = $1"#,
        )
        .bind(exchange_id)
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_sunrise_rule_applied(&self, exchange_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET sunrise_rule_applied = true, status = 'completed',
                   pending_travel_rule = false, updated_at = NOW()
               WHERE exchange_id = $1"#,
        )
        .bind(exchange_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_screening_failed(
        &self,
        exchange_id: Uuid,
        screening_result: serde_json::Value,
        compliance_case_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET status = 'manual_review',
                   screening_result = $2,
                   compliance_case_id = $3,
                   pending_travel_rule = true,
                   updated_at = NOW()
               WHERE exchange_id = $1"#,
        )
        .bind(exchange_id)
        .bind(&screening_result)
        .bind(compliance_case_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn increment_retry(&self, exchange_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET retry_count = retry_count + 1, last_retry_at = NOW(),
                   status = 'pending', updated_at = NOW()
               WHERE exchange_id = $1"#,
        )
        .bind(exchange_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns pending outbound exchanges whose SLA window has elapsed.
    pub async fn get_unacknowledged_past_sla(&self) -> Result<Vec<TravelRuleExchange>> {
        let exchanges = sqlx::query_as::<_, TravelRuleExchange>(
            r#"SELECT * FROM travel_rule_exchanges
               WHERE status = 'pending'
                 AND direction = 'outbound'
                 AND timeout_at < NOW()
                 AND sla_breached = false"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(exchanges)
    }

    pub async fn mark_sla_breached(&self, exchange_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET status = 'timed_out', sla_breached = true,
                   sla_breached_at = NOW(), pending_travel_rule = false,
                   updated_at = NOW()
               WHERE exchange_id = $1"#,
        )
        .bind(exchange_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_unacknowledged_outbound(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM travel_rule_exchanges
               WHERE status = 'pending' AND direction = 'outbound'"#,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn count_pending_inbound_screening(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM travel_rule_exchanges
               WHERE status IN ('pending','manual_review') AND direction = 'inbound'"#,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    // -------------------------------------------------------------------------
    // Thresholds
    // -------------------------------------------------------------------------

    pub async fn get_threshold(
        &self,
        currency: &str,
        transaction_type: &str,
        jurisdiction: &str,
    ) -> Result<Option<TravelRuleThreshold>> {
        let threshold = sqlx::query_as::<_, TravelRuleThreshold>(
            r#"SELECT * FROM travel_rule_thresholds
               WHERE currency = $1
                 AND transaction_type = $2
                 AND jurisdiction = $3
                 AND is_active = true
               LIMIT 1"#,
        )
        .bind(currency)
        .bind(transaction_type)
        .bind(jurisdiction)
        .fetch_optional(&self.pool)
        .await?;
        Ok(threshold)
    }

    pub async fn list_thresholds(&self) -> Result<Vec<TravelRuleThreshold>> {
        let thresholds = sqlx::query_as::<_, TravelRuleThreshold>(
            "SELECT * FROM travel_rule_thresholds ORDER BY currency, transaction_type, jurisdiction",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(thresholds)
    }

    pub async fn upsert_threshold(&self, item: &ThresholdUpdateItem, approved_by: Uuid) -> Result<TravelRuleThreshold> {
        let threshold = sqlx::query_as::<_, TravelRuleThreshold>(
            r#"INSERT INTO travel_rule_thresholds
                (currency, transaction_type, jurisdiction, threshold_amount, is_active, approved_by, approved_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,NOW(),NOW())
               ON CONFLICT (currency, transaction_type, jurisdiction)
               DO UPDATE SET
                   threshold_amount = EXCLUDED.threshold_amount,
                   is_active = EXCLUDED.is_active,
                   approved_by = EXCLUDED.approved_by,
                   approved_at = NOW(),
                   updated_at = NOW()
               RETURNING *"#,
        )
        .bind(&item.currency)
        .bind(&item.transaction_type)
        .bind(&item.jurisdiction)
        .bind(item.threshold_amount)
        .bind(item.is_active)
        .bind(approved_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(threshold)
    }

    // -------------------------------------------------------------------------
    // Unhosted wallet policy
    // -------------------------------------------------------------------------

    pub async fn get_active_policy(&self) -> Result<Option<UnhostedWalletPolicy>> {
        let policy = sqlx::query_as::<_, UnhostedWalletPolicy>(
            "SELECT * FROM travel_rule_unhosted_wallet_policy ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(policy)
    }

    pub async fn update_policy(&self, req: &UpdateUnhostedWalletPolicyRequest) -> Result<UnhostedWalletPolicy> {
        // Insert a new row (full audit trail — old rows remain for history)
        let policy = sqlx::query_as::<_, UnhostedWalletPolicy>(
            r#"INSERT INTO travel_rule_unhosted_wallet_policy
                (policy_type, threshold_amount, threshold_currency,
                 updated_by_compliance_officer, updated_by_senior_management,
                 created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,NOW(),NOW())
               RETURNING *"#,
        )
        .bind(&req.policy_type)
        .bind(req.threshold_amount)
        .bind(&req.threshold_currency)
        .bind(req.compliance_officer_id)
        .bind(req.senior_management_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(policy)
    }

    // -------------------------------------------------------------------------
    // Attestations
    // -------------------------------------------------------------------------

    pub async fn create_attestation(&self, req: &SelfAttestationRequest) -> Result<TravelRuleAttestation> {
        let attestation = sqlx::query_as::<_, TravelRuleAttestation>(
            r#"INSERT INTO travel_rule_attestations
                (user_id, transaction_id, wallet_address, ip_address, user_agent)
               VALUES ($1,$2,$3,$4,$5)
               RETURNING *"#,
        )
        .bind(req.user_id)
        .bind(&req.transaction_id)
        .bind(&req.wallet_address)
        .bind(&req.ip_address)
        .bind(&req.user_agent)
        .fetch_one(&self.pool)
        .await?;
        Ok(attestation)
    }

    pub async fn get_attestation_by_transaction(&self, transaction_id: &str) -> Result<Option<TravelRuleAttestation>> {
        let att = sqlx::query_as::<_, TravelRuleAttestation>(
            "SELECT * FROM travel_rule_attestations WHERE transaction_id = $1 ORDER BY attested_at DESC LIMIT 1",
        )
        .bind(transaction_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(att)
    }

    // -------------------------------------------------------------------------
    // Metrics aggregation
    // -------------------------------------------------------------------------

    pub async fn compute_metrics(
        &self,
        from: chrono::DateTime<Utc>,
        to: chrono::DateTime<Utc>,
    ) -> Result<TravelRuleMetrics> {
        let (total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        let (successful,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE status = 'acknowledged' AND created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        let (failed,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE status = 'failed' AND created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        let (sunrise,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE sunrise_rule_applied = true AND created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        let (inbound,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE direction = 'inbound' AND created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        let (screening_failures,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE direction = 'inbound' AND status = 'manual_review' AND compliance_case_id IS NOT NULL AND created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        // Protocol breakdown for outbound
        let proto_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT protocol_used::TEXT, COUNT(*) AS cnt
               FROM travel_rule_exchanges
               WHERE direction = 'outbound' AND created_at BETWEEN $1 AND $2
               GROUP BY protocol_used"#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;

        let outbound_by_protocol: HashMap<String, i64> = proto_rows.into_iter().collect();

        let registry_size = self.vasp_registry_size().await.unwrap_or(0);
        let unacknowledged = self.count_unacknowledged_outbound().await.unwrap_or(0);
        let pending_inbound = self.count_pending_inbound_screening().await.unwrap_or(0);

        // Unhosted wallet transactions = those whose beneficiary_vasp_id is 'unhosted'
        let (unhosted,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM travel_rule_exchanges WHERE beneficiary_vasp_id = 'unhosted' AND created_at BETWEEN $1 AND $2",
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        Ok(TravelRuleMetrics {
            period_from: from,
            period_to: to,
            total_threshold_triggering: total,
            successful_exchanges: successful,
            failed_exchanges: failed,
            unhosted_wallet_transactions: unhosted,
            sunrise_rule_applications: sunrise,
            inbound_received: inbound,
            inbound_screening_failures: screening_failures,
            outbound_by_protocol,
            vasp_registry_size: registry_size,
            unacknowledged_outbound: unacknowledged,
            pending_inbound_screening: pending_inbound,
        })
    }
}
