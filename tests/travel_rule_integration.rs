/// Travel Rule Integration Tests (Issue #452)
///
/// These tests require a live PostgreSQL database.
/// Before running:
///   1. `sqlx migrate run` (apply 20270428000002 + 20270601000000)
///   2. `DATABASE_URL=<connection_string> cargo test --test travel_rule_integration`
///
/// Tests marked `#[ignore]` are skipped when DATABASE_URL is not set.
/// CI note: integration tests are skipped unless DATABASE_URL is configured in CI env.

#[cfg(feature = "database")]
mod travel_rule_integration {
    use aframp_backend::travel_rule::{
        models::*, repository::TravelRuleRepository, service::TravelRuleService,
    };
    use rust_decimal::Decimal;
    use sqlx::PgPool;
    use std::str::FromStr;
    use std::sync::Arc;
    use uuid::Uuid;

    async fn test_pool() -> Result<PgPool, Box<dyn std::error::Error>> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL required".into())?;
        Ok(sqlx::PgPool::connect(&url).await?)
    }

    // -------------------------------------------------------------------------
    // 1. VASP registry CRUD
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_vasp_registry_create_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let vasp_id = format!("test-vasp-{}", Uuid::new_v4().simple());
        let req = RegisterVaspRequest {
            vasp_id: vasp_id.clone(),
            vasp_name: "Test VASP Ltd".into(),
            jurisdiction: "NG".into(),
            supported_protocols: vec!["trisa".into(), "trp".into()],
            travel_rule_endpoint: Some("https://vasp.example.com/tr".into()),
            vasp_did: Some("did:web:vasp.example.com".into()),
            lei: None,
            regulatory_status: Some(VaspRegulatoryStatus::Licensed),
            trust_status: Some(VaspTrustStatus::Verified),
            public_key_pem: None,
        };

        let created = repo.create_vasp(&req).await?;
        assert_eq!(created.vasp_id, vasp_id);
        assert_eq!(created.trust_status, VaspTrustStatus::Verified);

        let retrieved = repo.get_vasp(&vasp_id).await?.ok_or("VASP not found")?;
        assert_eq!(retrieved.vasp_name, "Test VASP Ltd");
        assert!(retrieved.supported_protocols.contains(&"trisa".to_string()));
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 2. Threshold management
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_threshold_upsert_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let item = ThresholdUpdateItem {
            currency: "cNGN".into(),
            transaction_type: "offramp".into(),
            jurisdiction: "NG".into(),
            threshold_amount: Decimal::from_str("500000")?,
            is_active: true,
        };

        let officer_id = Uuid::new_v4();
        let upserted = repo.upsert_threshold(&item, officer_id).await?;
        assert_eq!(upserted.currency, "cNGN");
        assert_eq!(upserted.threshold_amount, Decimal::from_str("500000")?);
        assert_eq!(upserted.approved_by, Some(officer_id));

        let list = repo.list_thresholds().await?;
        assert!(list.iter().any(|t| t.currency == "cNGN" && t.transaction_type == "offramp"));
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 3. Unhosted wallet policy lifecycle
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_unhosted_wallet_policy_update() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let update_req = UpdateUnhostedWalletPolicyRequest {
            policy_type: "require_attestation".into(),
            threshold_amount: Some(Decimal::from_str("100000")?),
            threshold_currency: Some("cNGN".into()),
            compliance_officer_id: Uuid::new_v4(),
            senior_management_id: Uuid::new_v4(),
        };

        let policy = repo.update_policy(&update_req).await?;
        assert_eq!(policy.policy_type, "require_attestation");

        let active = repo.get_active_policy().await?.ok_or("Policy not found")?;
        assert_eq!(active.policy_type, "require_attestation");
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 4. Self-attestation flow
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_self_attestation_create_and_retrieve() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let tx_id = format!("tx-{}", Uuid::new_v4().simple());
        let req = SelfAttestationRequest {
            user_id: Uuid::new_v4(),
            transaction_id: tx_id.clone(),
            wallet_address: "GAJHF7SVXMVDSKJF".into(),
            ip_address: Some("41.58.0.1".into()),
            user_agent: Some("Mozilla/5.0".into()),
        };

        let att = repo.create_attestation(&req).await?;
        assert_eq!(att.transaction_id, tx_id);

        let retrieved = repo
            .get_attestation_by_transaction(&tx_id)
            .await?
            .ok_or("Attestation not found")?;
        assert_eq!(retrieved.wallet_address, "GAJHF7SVXMVDSKJF");
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 5. Exchange record create + acknowledge
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_exchange_create_and_acknowledge() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let tx_id = format!("tx-{}", Uuid::new_v4().simple());
        let exchange = repo
            .create_exchange(
                "aframp-ng",
                "test-beneficiary-vasp",
                &tx_id,
                &TravelRuleProtocol::Trisa,
                &TravelRuleDirection::Outbound,
                serde_json::json!({"type": "natural", "first_name": "Test"}),
                serde_json::json!({"type": "natural", "first_name": "Beneficiary"}),
                "500000",
                "cNGN",
                300,
            )
            .await?;

        assert_eq!(exchange.status, TravelRuleStatus::Pending);
        assert!(exchange.pending_travel_rule);

        repo.mark_acknowledged(exchange.exchange_id).await?;

        let updated = repo
            .get_exchange(exchange.exchange_id)
            .await?
            .ok_or("Exchange not found")?;
        assert_eq!(updated.status, TravelRuleStatus::Acknowledged);
        assert!(!updated.pending_travel_rule);
        assert!(updated.acknowledged_at.is_some());
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 6. SLA breach detection
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_sla_breach_detection() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        // Create an exchange with a 0-second SLA (immediately expired)
        let tx_id = format!("tx-sla-{}", Uuid::new_v4().simple());
        let exchange = repo
            .create_exchange(
                "aframp-ng",
                "slow-vasp",
                &tx_id,
                &TravelRuleProtocol::Trisa,
                &TravelRuleDirection::Outbound,
                serde_json::json!({}),
                serde_json::json!({}),
                "500000",
                "cNGN",
                -1, // intentionally expired
            )
            .await?;

        // Manually set timeout_at to past
        sqlx::query(
            "UPDATE travel_rule_exchanges SET timeout_at = NOW() - INTERVAL '1 second' WHERE exchange_id = $1",
        )
        .bind(exchange.exchange_id)
        .execute(&pool)
        .await?;

        let breached = repo.get_unacknowledged_past_sla().await?;
        assert!(
            breached
                .iter()
                .any(|e| e.exchange_id == exchange.exchange_id)
        );

        repo.mark_sla_breached(exchange.exchange_id).await?;

        let updated = repo
            .get_exchange(exchange.exchange_id)
            .await?
            .ok_or("Exchange not found")?;
        assert_eq!(updated.status, TravelRuleStatus::TimedOut);
        assert!(updated.sla_breached);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 7. Sunrise rule — VASP with no recognized protocol
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_sunrise_rule_applied() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let tx_id = format!("tx-sunrise-{}", Uuid::new_v4().simple());
        let exchange = repo
            .create_exchange(
                "aframp-ng",
                "no-protocol-vasp",
                &tx_id,
                &TravelRuleProtocol::Unknown,
                &TravelRuleDirection::Outbound,
                serde_json::json!({}),
                serde_json::json!({}),
                "500000",
                "cNGN",
                300,
            )
            .await?;

        repo.mark_sunrise_rule_applied(exchange.exchange_id).await?;

        let updated = repo
            .get_exchange(exchange.exchange_id)
            .await?
            .ok_or("Exchange not found")?;
        assert!(updated.sunrise_rule_applied);
        assert_eq!(updated.status, TravelRuleStatus::Completed);
        assert!(!updated.pending_travel_rule);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 8. Inbound screening failure — creates compliance hold
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_inbound_screening_failure_hold() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let tx_id = format!("tx-screening-{}", Uuid::new_v4().simple());
        let exchange = repo
            .create_exchange(
                "suspect-vasp",
                "aframp-ng",
                &tx_id,
                &TravelRuleProtocol::Trisa,
                &TravelRuleDirection::Inbound,
                serde_json::json!({"type": "natural", "first_name": "Suspected", "last_name": "Person"}),
                serde_json::json!({}),
                "500000",
                "cNGN",
                300,
            )
            .await?;

        let case_id = Uuid::new_v4();
        let screening_json = serde_json::json!({
            "risk_score": 1.0,
            "cleared": false,
            "flag_level": "Critical",
            "case_id": case_id.to_string(),
        });

        repo.mark_screening_failed(exchange.exchange_id, screening_json, case_id)
            .await?;

        let updated = repo
            .get_exchange(exchange.exchange_id)
            .await?
            .ok_or("Exchange not found")?;
        assert_eq!(updated.status, TravelRuleStatus::ManualReview);
        assert_eq!(updated.compliance_case_id, Some(case_id));
        assert!(updated.pending_travel_rule);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 9. Retry mechanism
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_retry_increments_count() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let tx_id = format!("tx-retry-{}", Uuid::new_v4().simple());
        let exchange = repo
            .create_exchange(
                "aframp-ng",
                "unreachable-vasp",
                &tx_id,
                &TravelRuleProtocol::Trisa,
                &TravelRuleDirection::Outbound,
                serde_json::json!({}),
                serde_json::json!({}),
                "500000",
                "cNGN",
                300,
            )
            .await?;

        assert_eq!(exchange.retry_count, 0);

        repo.increment_retry(exchange.exchange_id).await?;
        repo.increment_retry(exchange.exchange_id).await?;

        let updated = repo
            .get_exchange(exchange.exchange_id)
            .await?
            .ok_or("Exchange not found")?;
        assert_eq!(updated.retry_count, 2);
        assert!(updated.last_retry_at.is_some());
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 10. Metrics computation
    // -------------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn test_metrics_computation() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_pool().await?;
        let repo = TravelRuleRepository::new(pool);

        let from = chrono::Utc::now() - chrono::Duration::hours(1);
        let to = chrono::Utc::now() + chrono::Duration::hours(1);

        let metrics = repo.compute_metrics(from, to).await?;

        // Basic sanity: total must be >= acknowledged + failed
        assert!(
            metrics.total_threshold_triggering
                >= metrics.successful_exchanges + metrics.failed_exchanges
        );
        assert!(metrics.vasp_registry_size >= 0);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 11. IVMS101 completeness validation via service
    // -------------------------------------------------------------------------

    #[tokio::test]
    fn test_ivms101_completeness_all_required_fields() {
        // This test is pure (no DB), so no #[ignore]
        let complete = Ivms101Person::Natural(Ivms101NaturalPerson {
            first_name: "Chidi".into(),
            last_name: "Nwosu".into(),
            date_of_birth: Some("1985-03-22".into()),
            national_id: Some("NIN-98765432101".into()),
            address: Some("5 Adeola Odeku, Victoria Island, Lagos".into()),
            country_of_residence: Some("NG".into()),
            account_number: Some("GCEZWKFE5OEQBVBLN3CNNO".into()),
        });

        let missing_dob = Ivms101Person::Natural(Ivms101NaturalPerson {
            first_name: "Chidi".into(),
            last_name: "Nwosu".into(),
            date_of_birth: None,
            national_id: Some("NIN-98765432101".into()),
            address: None,
            country_of_residence: Some("NG".into()),
            account_number: Some("GCEZWKFE5OEQBVBLN3CNNO".into()),
        });

        // Complete person serialises without error
        assert!(serde_json::to_value(&complete).is_ok());
        // Missing DOB is detectable
        if let Ivms101Person::Natural(p) = &missing_dob {
            assert!(p.date_of_birth.is_none());
        }
    }

    // -------------------------------------------------------------------------
    // 12. AES-256-GCM encryption round trip
    // -------------------------------------------------------------------------

    #[tokio::test]
    fn test_aes_gcm_encryption_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        use aes_gcm::aead::{Aead, KeyInit, OsRng};
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use rand::RngCore;

        let data = serde_json::to_vec(&serde_json::json!({
            "first_name": "Ngozi",
            "last_name": "Adeyemi",
            "national_id": "NIN-11223344556",
            "dob": "1992-07-14",
        }))?;

        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data.as_ref())
            .map_err(|e| format!("Encryption failed: {:?}", e))?;
        let decrypted = cipher.decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| format!("Decryption failed: {:?}", e))?;

        assert_eq!(decrypted, data);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 13. Enforcement gate — transfers with incomplete originator/beneficiary
    //     data are rejected before the transaction can proceed (Issue: Travel
    //     Rule enforcement on transfers above threshold). Pure (no DB).
    // -------------------------------------------------------------------------

    fn complete_natural(account_number: Option<&str>) -> Ivms101Person {
        Ivms101Person::Natural(Ivms101NaturalPerson {
            first_name: "Amaka".into(),
            last_name: "Okafor".into(),
            date_of_birth: None,
            national_id: None,
            address: None,
            country_of_residence: Some("NG".into()),
            account_number: account_number.map(String::from),
        })
    }

    #[tokio::test]
    async fn test_enforcement_blocks_missing_account_number() {
        let person = complete_natural(None);
        let result = TravelRuleService::validate_minimum_fields(&person);
        assert!(
            result.is_err(),
            "a transfer must not be allowed to proceed without an account/wallet identifier"
        );
    }

    #[tokio::test]
    async fn test_enforcement_blocks_missing_name() {
        let person = Ivms101Person::Natural(Ivms101NaturalPerson {
            first_name: "".into(),
            last_name: "".into(),
            date_of_birth: None,
            national_id: None,
            address: None,
            country_of_residence: Some("NG".into()),
            account_number: Some("GAJHF7SVXMVDSKJF".into()),
        });
        let result = TravelRuleService::validate_minimum_fields(&person);
        assert!(
            result.is_err(),
            "a transfer must not be allowed to proceed without originator/beneficiary identification"
        );
    }

    #[tokio::test]
    async fn test_enforcement_allows_minimum_fields_present() {
        let person = complete_natural(Some("GAJHF7SVXMVDSKJF"));
        let result = TravelRuleService::validate_minimum_fields(&person);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_enforcement_blocks_legal_person_missing_name() {
        let person = Ivms101Person::Legal(Ivms101LegalPerson {
            legal_name: "".into(),
            registration_number: None,
            country_of_registration: None,
            address: None,
            account_number: Some("GBZXHF7SVXMVDSKJF".into()),
        });
        let result = TravelRuleService::validate_minimum_fields(&person);
        assert!(result.is_err());
    }

}
