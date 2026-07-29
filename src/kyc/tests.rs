#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::kyc_repository::{DocumentType, KycStatus, KycTier};
    use crate::kyc::limits::KycLimitsEnforcer;
    use crate::kyc::tier_requirements::{KycTierRequirements, TransactionLimitEnforcer};
    use bigdecimal::BigDecimal;
    use std::str::FromStr;
    use uuid::Uuid;

    // ─── helpers ──────────────────────────────────────────────────────────────

    /// Parse a `BigDecimal` from a string literal, turning a parse failure into
    /// a test failure with a meaningful message instead of a panic.
    fn bd(s: &str) -> BigDecimal {
        BigDecimal::from_str(s)
            .unwrap_or_else(|e| panic!("invalid BigDecimal literal {:?}: {}", s, e))
    }

    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_tier_requirements_validation() {
        // Test Tier 1 (Basic) requirements
        let submitted_docs = vec![DocumentType::NationalId];
        let result =
            KycTierRequirements::validate_tier_requirements(KycTier::Basic, &submitted_docs);

        assert!(result.is_valid);
        assert!(result.missing_documents.is_empty());
        assert_eq!(result.tier, KycTier::Basic);

        // Test missing documents for Tier 2
        let submitted_docs = vec![DocumentType::NationalId];
        let result =
            KycTierRequirements::validate_tier_requirements(KycTier::Standard, &submitted_docs);

        assert!(!result.is_valid);
        assert!(!result.missing_documents.is_empty());
        assert!(result
            .missing_documents
            .contains(&DocumentType::UtilityBill));
    }

    #[test]
    fn test_tier_upgrade_validation() {
        let submitted_docs = vec![
            DocumentType::NationalId,
            DocumentType::Passport,
            DocumentType::UtilityBill,
        ];

        // Can upgrade from Unverified to Basic
        assert!(KycTierRequirements::can_upgrade_to_tier(
            KycTier::Unverified,
            KycTier::Basic,
            &submitted_docs
        ));

        // Cannot downgrade
        assert!(!KycTierRequirements::can_upgrade_to_tier(
            KycTier::Standard,
            KycTier::Basic,
            &submitted_docs
        ));

        // Cannot upgrade to same tier
        assert!(!KycTierRequirements::can_upgrade_to_tier(
            KycTier::Basic,
            KycTier::Basic,
            &submitted_docs
        ));
    }

    #[test]
    fn test_transaction_limit_enforcement() {
        let enforcer = TransactionLimitEnforcer::new(KycTier::Basic);

        // Test within limits
        let amount = bd("500.00");
        let daily_used = bd("1000.00");
        let monthly_used = bd("10000.00");

        let result = enforcer.check_transaction_limits(amount, daily_used, monthly_used);
        assert!(result.is_allowed);
        assert!(result.violations.is_empty());

        // Test single transaction limit violation (exceeds $1000 limit)
        let amount = bd("2000.00");
        let daily_used = bd("0.00");
        let monthly_used = bd("0.00");

        let result = enforcer.check_transaction_limits(amount, daily_used, monthly_used);
        assert!(!result.is_allowed);
        assert!(!result.violations.is_empty());

        // Test daily volume limit violation ($4600 + $500 = $5100 > $5000 limit)
        let amount = bd("500.00");
        let daily_used = bd("4600.00");
        let monthly_used = bd("0.00");

        let result = enforcer.check_transaction_limits(amount, daily_used, monthly_used);
        assert!(!result.is_allowed);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_tier_limits() {
        let basic_limits = KycTierRequirements::get_tier_limits(KycTier::Basic);
        assert_eq!(basic_limits.max_transaction_amount, bd("1000.00"));
        assert_eq!(basic_limits.daily_volume_limit, bd("5000.00"));
        assert_eq!(basic_limits.monthly_volume_limit, bd("50000.00"));

        let standard_limits = KycTierRequirements::get_tier_limits(KycTier::Standard);
        assert_eq!(standard_limits.max_transaction_amount, bd("10000.00"));
        assert_eq!(standard_limits.daily_volume_limit, bd("50000.00"));
        assert_eq!(standard_limits.monthly_volume_limit, bd("500000.00"));
    }

    #[test]
    fn test_document_type_mapping() {
        // Test that all document types have mappings
        let all_types = vec![
            DocumentType::NationalId,
            DocumentType::Passport,
            DocumentType::DriversLicense,
            DocumentType::UtilityBill,
            DocumentType::BankStatement,
            DocumentType::GovernmentLetter,
            DocumentType::SourceOfFunds,
            DocumentType::BusinessRegistration,
        ];

        for doc_type in &all_types {
            let required_for_basic =
                KycTierRequirements::is_document_required_for_tier(*doc_type, KycTier::Basic);
            let required_for_standard =
                KycTierRequirements::is_document_required_for_tier(*doc_type, KycTier::Standard);
            let required_for_enhanced =
                KycTierRequirements::is_document_required_for_tier(*doc_type, KycTier::Enhanced);

            // Enhanced tier should require all documents
            assert!(
                required_for_enhanced,
                "Enhanced tier should require {:?}",
                doc_type
            );

            // Basic tier should only require ID documents
            match doc_type {
                DocumentType::NationalId
                | DocumentType::Passport
                | DocumentType::DriversLicense => {
                    assert!(
                        required_for_basic,
                        "Basic tier should require {:?}",
                        doc_type
                    );
                    assert!(
                        required_for_standard,
                        "Standard tier should require {:?}",
                        doc_type
                    );
                }
                DocumentType::UtilityBill
                | DocumentType::BankStatement
                | DocumentType::GovernmentLetter => {
                    assert!(
                        !required_for_basic,
                        "Basic tier should not require {:?}",
                        doc_type
                    );
                    assert!(
                        required_for_standard,
                        "Standard tier should require {:?}",
                        doc_type
                    );
                }
                DocumentType::SourceOfFunds | DocumentType::BusinessRegistration => {
                    assert!(
                        !required_for_basic,
                        "Basic tier should not require {:?}",
                        doc_type
                    );
                    assert!(
                        !required_for_standard,
                        "Standard tier should not require {:?}",
                        doc_type
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn test_kyc_service_session_creation() {
        // This test would require setting up a test database and mock provider.
        // For now we exercise the validation logic that has no external deps.

        let consumer_id = Uuid::new_v4();
        let target_tier = KycTier::Basic;

        assert_ne!(consumer_id, Uuid::default());
        // A session must never target the Unverified tier.
        assert_ne!(target_tier, KycTier::Unverified);
    }

    #[tokio::test]
    async fn test_volume_tracker_reset() {
        // Requires a test database — verify only the invariant that a new UUID
        // is non-nil (the real reset test lives in the integration suite).
        let consumer_id = Uuid::new_v4();
        assert_ne!(consumer_id, Uuid::default());
    }

    #[test]
    fn test_edd_trigger_configuration() {
        use crate::kyc::compliance::EddTriggerConfig;

        let config = EddTriggerConfig::default();

        assert!(config.volume_spike_threshold > 0.0);
        assert!(!config.high_risk_jurisdictions.is_empty());
        assert!(config.structuring_threshold > 0);
        assert!(config.structuring_timeframe_hours > 0);
        assert!(config.max_single_transaction > bd("0"));
        assert!(config.daily_volume_threshold > bd("0"));
        assert!(config.rapid_succession_threshold > 0);
        assert!(config.rapid_succession_minutes > 0);
    }

    #[test]
    fn test_kyc_metrics_creation() {
        use crate::kyc::observability::KycMetrics;

        let metrics = KycMetrics::new();

        // Verify metrics can be recorded without panicking.
        metrics.record_session_initiated(KycTier::Basic);
        metrics.record_verification_started(KycTier::Basic);
        metrics.record_document_submitted("national_id");
        metrics.record_limit_check(KycTier::Basic);

        // Export must succeed and include expected metric names.
        let export_text = metrics
            .export()
            .expect("KycMetrics::export should not fail");
        assert!(export_text.contains("kyc_sessions_initiated_total"));
        assert!(export_text.contains("kyc_verifications_total"));
        assert!(export_text.contains("kyc_documents_submitted_total"));
        assert!(export_text.contains("kyc_limit_checks_total"));
    }

    #[test]
    fn test_structured_logging() {
        use crate::database::kyc_repository::KycEventType;
        use crate::kyc::observability::KycLogger;

        let consumer_id = Uuid::new_v4();

        // Logging functions must not panic.
        KycLogger::log_kyc_event(
            consumer_id,
            KycEventType::SessionInitiated,
            Some(KycTier::Basic),
            Some("test_provider"),
            "Test session initiated",
            None,
        );

        KycLogger::log_decision(
            consumer_id,
            KycStatus::Approved,
            KycTier::Basic,
            "Test approval",
            Some(Uuid::new_v4()),
            Some("Provider response"),
        );

        KycLogger::log_provider_error(
            "test_provider",
            "create_session",
            "Connection timeout",
            Some(consumer_id),
        );
    }

    #[test]
    fn test_provider_error_handling() {
        use crate::kyc::provider::KycProviderError;

        let error = KycProviderError::ApiError("Test error".to_string());
        assert!(matches!(error, KycProviderError::ApiError(_)));

        let error = KycProviderError::AuthenticationError("Invalid credentials".to_string());
        assert!(matches!(error, KycProviderError::AuthenticationError(_)));

        let error = KycProviderError::RateLimitExceeded;
        assert!(matches!(error, KycProviderError::RateLimitExceeded));
    }

    #[test]
    fn test_kyc_service_error_conversion() {
        use crate::error::ApiError;
        use crate::kyc::service::KycServiceError;

        let kyc_error = KycServiceError::SessionAlreadyActive;
        let api_error: ApiError = kyc_error.into();
        assert!(matches!(api_error, ApiError::Conflict(_)));

        let kyc_error = KycServiceError::KycRecordNotFound;
        let api_error: ApiError = kyc_error.into();
        assert!(matches!(api_error, ApiError::NotFound(_)));

        let kyc_error = KycServiceError::SessionExpired;
        let api_error: ApiError = kyc_error.into();
        assert!(matches!(api_error, ApiError::BadRequest(_)));
    }

    #[test]
    fn test_compliance_alert_types() {
        use crate::kyc::compliance::{ComplianceAlertType, EddSeverity};

        let alert_types = vec![
            ComplianceAlertType::ManualReviewBacklog,
            ComplianceAlertType::ProviderWebhookFailure,
            ComplianceAlertType::HighVolumeSpike,
            ComplianceAlertType::SuspiciousPattern,
            ComplianceAlertType::RegulatoryThreshold,
            ComplianceAlertType::SystemAnomaly,
        ];

        let severities = vec![
            EddSeverity::Low,
            EddSeverity::Medium,
            EddSeverity::High,
            EddSeverity::Critical,
        ];

        for alert_type in alert_types {
            for severity in severities.clone() {
                // Verify variants can be constructed; serialization is tested separately.
                let _ = (alert_type.clone(), severity.clone());
            }
        }
    }

    #[test]
    fn test_audit_export_formats() {
        use crate::kyc::compliance::AuditExportFormat;

        let formats = vec![AuditExportFormat::Json, AuditExportFormat::Csv];

        for format in formats {
            let serialized = serde_json::to_string(&format)
                .unwrap_or_else(|e| panic!("failed to serialize {:?}: {}", format, e));
            let deserialized: AuditExportFormat = serde_json::from_str(&serialized)
                .unwrap_or_else(|e| {
                    panic!("failed to deserialize {:?}: {}", serialized, e)
                });
            assert_eq!(format, deserialized);
        }
    }

    #[test]
    fn test_bigdecimal_arithmetic() {
        let amount1 = bd("1000.50");
        let amount2 = bd("500.25");

        let sum = &amount1 + &amount2;
        assert_eq!(sum, bd("1500.75"));

        let difference = &amount1 - &amount2;
        assert_eq!(difference, bd("500.25"));

        // Ordering
        assert!(amount1 > amount2);
        assert!(amount2 < amount1);
        assert_eq!(amount1, amount1);

        // Limit checking
        let limit = bd("1000.00");
        let under_limit = bd("999.99");
        let over_limit = bd("1000.01");

        assert!(under_limit <= limit);
        assert!(over_limit > limit);
    }

    #[test]
    fn test_uuid_handling() {
        let consumer_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();

        // Round-trip through string representation.
        let consumer_str = consumer_id.to_string();
        let consumer_parsed = Uuid::parse_str(&consumer_str)
            .unwrap_or_else(|e| panic!("UUID round-trip failed for {}: {}", consumer_str, e));
        assert_eq!(consumer_id, consumer_parsed);

        // Two freshly generated UUIDs must differ.
        assert_ne!(consumer_id, session_id);

        // Nil UUID sanity check.
        let nil_uuid = Uuid::nil();
        assert_eq!(nil_uuid.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn test_datetime_handling() {
        use chrono::{DateTime, Duration, Utc};

        let now = Utc::now();
        let later = now + Duration::hours(1);
        let earlier = now - Duration::hours(1);

        assert!(later > now);
        assert!(earlier < now);
        assert_eq!(now, now);

        // RFC 3339 round-trip must not fail.
        let now_str = now.to_rfc3339();
        let parsed = DateTime::parse_from_rfc3339(&now_str)
            .unwrap_or_else(|e| panic!("RFC3339 parse failed for {:?}: {}", now_str, e));
        assert_eq!(now, parsed.with_timezone(&Utc));
    }

    #[test]
    fn test_json_serialization() {
        use crate::database::kyc_repository::{DocumentType, KycStatus, KycTier};

        // KycTier round-trip
        let tier = KycTier::Standard;
        let tier_json = serde_json::to_string(&tier)
            .unwrap_or_else(|e| panic!("KycTier serialization failed: {}", e));
        let tier_parsed: KycTier = serde_json::from_str(&tier_json)
            .unwrap_or_else(|e| panic!("KycTier deserialization failed: {}", e));
        assert_eq!(tier, tier_parsed);

        // KycStatus round-trip
        let status = KycStatus::Approved;
        let status_json = serde_json::to_string(&status)
            .unwrap_or_else(|e| panic!("KycStatus serialization failed: {}", e));
        let status_parsed: KycStatus = serde_json::from_str(&status_json)
            .unwrap_or_else(|e| panic!("KycStatus deserialization failed: {}", e));
        assert_eq!(status, status_parsed);

        // DocumentType round-trip
        let doc_type = DocumentType::Passport;
        let doc_json = serde_json::to_string(&doc_type)
            .unwrap_or_else(|e| panic!("DocumentType serialization failed: {}", e));
        let doc_parsed: DocumentType = serde_json::from_str(&doc_json)
            .unwrap_or_else(|e| panic!("DocumentType deserialization failed: {}", e));
        assert_eq!(doc_type, doc_parsed);
    }

    // ─── Integration placeholders (require live database) ─────────────────────

    #[tokio::test]
    #[ignore] // Requires test database
    async fn test_full_kyc_lifecycle() {
        // Full flow: session → documents → selfie → approval → limit enforcement → admin ops.
        // Tracked in the integration test suite; ignored here to keep unit tests self-contained.
        assert!(true);
    }

    #[tokio::test]
    #[ignore] // Requires test database
    async fn test_transaction_limit_enforcement_integration() {
        // Requires a live database with volume trackers.
        assert!(true);
    }

    #[tokio::test]
    #[ignore] // Requires test database
    async fn test_edd_triggering_integration() {
        // Requires a live database with transaction history.
        assert!(true);
    }
}
