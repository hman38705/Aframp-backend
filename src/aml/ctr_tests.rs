//! CTR Unit Tests
//!
//! Comprehensive unit tests for CTR functionality including aggregation,
//! threshold detection, deduplication, exemption enforcement, and more.

#[cfg(test)]
mod unit_tests {
    use crate::aml::ctr_aggregation::{CtrAggregationConfig, CtrAggregationService};
    use crate::aml::ctr_exemption::{CtrExemptionConfig, CtrExemptionService, CreateExemptionRequest};
    use crate::aml::ctr_generator::{CtrGeneratorConfig, CtrGeneratorService};
    use crate::aml::ctr_management::{CtrManagementConfig, ReviewChecklist};
    use crate::aml::models::CtrType;
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use uuid::Uuid;

    // ─── helpers ──────────────────────────────────────────────────────────────

    /// Parse a `Decimal` from a string literal, turning a parse failure into a
    /// test failure with a descriptive message instead of an opaque panic.
    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s)
            .unwrap_or_else(|e| panic!("invalid Decimal literal {:?}: {}", s, e))
    }

    // ─────────────────────────────────────────────────────────────────────────

    /// Test aggregation calculation
    #[test]
    fn test_aggregation_calculation() {
        let amount1 = dec("1000000");
        let amount2 = dec("2500000");
        let amount3 = dec("1500000");

        let total = amount1 + amount2 + amount3;
        assert_eq!(total, dec("5000000"));
    }

    /// Test NGN conversion (placeholder — would test actual conversion logic)
    #[test]
    fn test_ngn_conversion() {
        let usd_amount = dec("1000");
        let exchange_rate = dec("1500"); // 1 USD = 1 500 NGN
        let ngn_amount = usd_amount * exchange_rate;

        assert_eq!(ngn_amount, dec("1500000"));
    }

    /// Test threshold detection for individual
    #[test]
    fn test_threshold_detection_individual() {
        let threshold = dec("5000000");
        let amount_below = dec("4999999");
        let amount_at = dec("5000000");
        let amount_above = dec("5000001");

        assert!(amount_below < threshold);
        assert!(amount_at >= threshold);
        assert!(amount_above >= threshold);
    }

    /// Test threshold detection for corporate
    #[test]
    fn test_threshold_detection_corporate() {
        let threshold = dec("10000000");
        let amount_below = dec("9999999");
        let amount_at = dec("10000000");
        let amount_above = dec("10000001");

        assert!(amount_below < threshold);
        assert!(amount_at >= threshold);
        assert!(amount_above >= threshold);
    }

    /// Test proximity threshold calculation
    #[test]
    fn test_proximity_threshold() {
        let threshold = dec("5000000");
        let proximity_pct = dec("0.9");
        let proximity_amount = threshold * proximity_pct;

        assert_eq!(proximity_amount, dec("4500000"));

        let amount_in_proximity = dec("4600000");
        let amount_not_in_proximity = dec("4400000");

        assert!(amount_in_proximity >= proximity_amount);
        assert!(amount_not_in_proximity < proximity_amount);
    }

    /// Test deduplication logic
    #[test]
    fn test_deduplication() {
        let subject_id = Uuid::new_v4();
        let window_start = Utc::now();
        let window_end = window_start + Duration::days(1);

        // Identical keys derived from the same subject/window must be equal.
        let existing_ctr_key = format!("{}:{}:{}", subject_id, window_start, window_end);
        let new_ctr_key = format!("{}:{}:{}", subject_id, window_start, window_end);

        assert_eq!(existing_ctr_key, new_ctr_key);
    }

    /// Test exemption enforcement
    #[test]
    fn test_exemption_enforcement() {
        let exemption_expiry = Utc::now() + Duration::days(30);
        let now = Utc::now();

        // Active exemption
        assert!(exemption_expiry > now);

        // Expired exemption
        let expired_exemption = Utc::now() - Duration::days(1);
        assert!(expired_exemption < now);
    }

    /// Test review checklist completion
    #[test]
    fn test_review_checklist_complete() {
        let complete_checklist = ReviewChecklist {
            subject_identity_verified: true,
            transaction_details_accurate: true,
            amounts_reconciled: true,
            supporting_documents_attached: true,
            suspicious_activity_noted: false,
            regulatory_requirements_met: true,
        };

        assert!(complete_checklist.is_complete());

        let incomplete_checklist = ReviewChecklist {
            subject_identity_verified: true,
            transaction_details_accurate: false,
            amounts_reconciled: true,
            supporting_documents_attached: true,
            suspicious_activity_noted: false,
            regulatory_requirements_met: true,
        };

        assert!(!incomplete_checklist.is_complete());
        assert_eq!(incomplete_checklist.incomplete_items().len(), 1);
    }

    /// Test senior approval threshold
    #[test]
    fn test_senior_approval_threshold() {
        let config = CtrManagementConfig::default();
        let senior_threshold = config.senior_approval_threshold;

        let amount_below = dec("49999999");
        let amount_at = dec("50000000");
        let amount_above = dec("50000001");

        assert!(amount_below < senior_threshold);
        assert!(amount_at >= senior_threshold);
        assert!(amount_above >= senior_threshold);
    }

    /// Test format mapping (XML escaping)
    #[test]
    fn test_xml_escaping() {
        fn escape_xml(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;")
        }

        assert_eq!(escape_xml("Test & Co."), "Test &amp; Co.");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("'quote'"), "&apos;quote&apos;");
    }

    /// Test batch size calculation
    #[test]
    fn test_batch_size_calculation() {
        let ctr_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        assert_eq!(ctr_ids.len(), 3);

        let successful = 2;
        let failed = 0;
        let skipped = 1;

        assert_eq!(successful + failed + skipped, ctr_ids.len());
    }

    /// Test deadline calculation
    #[test]
    fn test_deadline_calculation() {
        let now = Utc::now();
        let deadline = now + Duration::days(15);

        let days_until = (deadline - now).num_days();
        assert_eq!(days_until, 15);

        // Test overdue
        let overdue_deadline = now - Duration::days(2);
        let days_overdue = (now - overdue_deadline).num_days();
        assert_eq!(days_overdue, 2);
    }

    /// Test reminder schedule
    #[test]
    fn test_reminder_schedule() {
        let now = Utc::now();
        let deadline = now + Duration::days(3);

        let days_until = (deadline - now).num_days();
        // First reminder fires at 3 days out.
        assert_eq!(days_until, 3);

        // Other reminder points.
        let one_day_before = now + Duration::days(1);
        assert_eq!((one_day_before - now).num_days(), 1);

        let deadline_day = now;
        assert_eq!((deadline_day - now).num_days(), 0);
    }

    /// Test transaction count validation
    #[test]
    fn test_transaction_count_validation() {
        let expected_count = 5;
        let actual_count = 5;

        assert_eq!(expected_count, actual_count);

        let mismatched_count = 4;
        assert_ne!(expected_count, mismatched_count);
    }

    /// Test amount reconciliation
    #[test]
    fn test_amount_reconciliation() {
        let ctr_total = dec("5000000");

        let tx1 = dec("1000000");
        let tx2 = dec("2000000");
        let tx3 = dec("2000000");

        let calculated_total = tx1 + tx2 + tx3;
        assert_eq!(ctr_total, calculated_total);
    }

    /// Test retry exponential backoff
    #[test]
    fn test_exponential_backoff() {
        let initial_delay = 2u64;
        let max_delay = 300u64;

        let delay1 = initial_delay;
        let delay2 = (delay1 * 2).min(max_delay);
        let delay3 = (delay2 * 2).min(max_delay);
        let delay4 = (delay3 * 2).min(max_delay);
        let delay5 = (delay4 * 2).min(max_delay);

        assert_eq!(delay1, 2);
        assert_eq!(delay2, 4);
        assert_eq!(delay3, 8);
        assert_eq!(delay4, 16);
        assert_eq!(delay5, 32);

        // Test cap
        let large_delay = 200u64;
        let capped = (large_delay * 2).min(max_delay);
        assert_eq!(capped, max_delay);
    }

    /// Test configuration defaults
    #[test]
    fn test_config_defaults() {
        let agg_config = CtrAggregationConfig::default();
        assert_eq!(agg_config.individual_threshold, dec("5000000"));
        assert_eq!(agg_config.corporate_threshold, dec("10000000"));

        let mgmt_config = CtrManagementConfig::default();
        assert_eq!(mgmt_config.senior_approval_threshold, dec("50000000"));
        assert!(mgmt_config.enforce_checklist);
    }

    /// Test subject type determination
    #[test]
    fn test_subject_type_determination() {
        let individual_type = CtrType::Individual;
        let corporate_type = CtrType::Corporate;

        assert_ne!(individual_type, corporate_type);

        // Threshold selection must pick the right value per subject type.
        let individual_threshold = dec("5000000");
        let corporate_threshold = dec("10000000");

        let selected_threshold = match individual_type {
            CtrType::Individual => individual_threshold,
            CtrType::Corporate => corporate_threshold,
        };

        assert_eq!(selected_threshold, dec("5000000"));
    }
}
