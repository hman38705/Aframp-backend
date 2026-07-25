//! Database-backed integration tests (require `integration` feature + DATABASE_URL)
//!
//! These tests mirror the mock-based tests in the sibling modules but use a
//! real PostgreSQL database via `BugBountyService` and `BugBountyRepository`.
//!
//! Run with:
//!   DATABASE_URL=postgres://... cargo test --features integration bug_bounty::db_integration_tests::

use prometheus::Registry;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;
use Bitmesh_backend::bug_bounty::{
    metrics::BugBountyMetrics, models::*, notifications::NotificationDispatcher,
    repository::BugBountyRepository, service::BugBountyService,
};

async fn make_service() -> BugBountyService {
    // INVARIANT: DATABASE_URL must be set; test cannot proceed without a DB connection.
    let url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
    // INVARIANT: DB must be reachable; unrecoverable if the connection fails at test startup.
    let pool = sqlx::PgPool::connect(&url).await.expect("db connect");
    let repo = Arc::new(BugBountyRepository::new(pool));
    let dispatcher = Arc::new(NotificationDispatcher::new(Arc::clone(&repo)));
    let config = BugBountyConfig::default();
    let registry = Registry::new();
    // INVARIANT: Metrics registration must succeed at startup; failure indicates a programming error.
    let metrics = Arc::new(BugBountyMetrics::new(&registry).expect("metrics"));
    BugBountyService::new(repo, dispatcher, config, metrics)
}

#[tokio::test]
async fn db_full_lifecycle() {
    let svc = make_service().await;
    let admin_id = Uuid::new_v4();

    let report = svc
        .create_report(
            CreateReportRequest {
                researcher_id: "db-alice".to_string(),
                severity: Severity::High,
                affected_component: "api/auth".to_string(),
                vulnerability_type: "sqli".to_string(),
                title: "DB lifecycle test".to_string(),
                description: "Integration test".to_string(),
                proof_of_concept: None,
                submission_content: serde_json::json!({}),
            },
            admin_id,
        )
        .await
        .expect("create report");

    assert_eq!(report.status, ReportStatus::New);

    let acked = svc
        .update_report(
            report.id,
            UpdateReportRequest {
                status: Some(ReportStatus::Acknowledged),
                severity: None,
                remediation_ref: None,
                coordinated_disclosure_date: None,
            },
            admin_id,
        )
        .await
        .expect("acknowledge");

    assert_eq!(acked.status, ReportStatus::Acknowledged);
    assert!(acked.acknowledged_at.is_some());

    let reward = svc
        .record_reward(
            report.id,
            RecordRewardRequest {
                amount_usd: Decimal::new(2000, 0),
                justification: "Valid finding".to_string(),
                escalation_justification: None,
            },
            admin_id,
        )
        .await
        .expect("record reward");

    assert_eq!(reward.amount_usd, Decimal::new(2000, 0));

    let resolved = svc
        .update_report(
            report.id,
            UpdateReportRequest {
                status: Some(ReportStatus::Resolved),
                severity: None,
                remediation_ref: None,
                coordinated_disclosure_date: None,
            },
            admin_id,
        )
        .await
        .expect("resolve");

    assert_eq!(resolved.status, ReportStatus::Resolved);
    assert!(resolved.resolved_at.is_some());
    assert!(resolved.coordinated_disclosure_date.is_some());
}

#[tokio::test]
async fn db_duplicate_detection() {
    let svc = make_service().await;
    let admin_id = Uuid::new_v4();

    let report_a = svc
        .create_report(
            CreateReportRequest {
                researcher_id: "db-alice".to_string(),
                severity: Severity::High,
                affected_component: "db-api/auth".to_string(),
                vulnerability_type: "db-sqli".to_string(),
                title: "Original".to_string(),
                description: "Original report".to_string(),
                proof_of_concept: None,
                submission_content: serde_json::json!({}),
            },
            admin_id,
        )
        .await
        .expect("create report A");

    assert_eq!(report_a.status, ReportStatus::New);

    let report_b = svc
        .create_report(
            CreateReportRequest {
                researcher_id: "db-bob".to_string(),
                severity: Severity::Critical,
                affected_component: "db-api/auth".to_string(),
                vulnerability_type: "db-sqli".to_string(),
                title: "Duplicate".to_string(),
                description: "Duplicate report".to_string(),
                proof_of_concept: None,
                submission_content: serde_json::json!({}),
            },
            admin_id,
        )
        .await
        .expect("create report B");

    assert_eq!(report_b.status, ReportStatus::Duplicate);
    assert_eq!(report_b.duplicate_of, Some(report_a.id));
}

#[tokio::test]
async fn db_monthly_cost_report() {
    let svc = make_service().await;
    let admin_id = Uuid::new_v4();

    let report = svc
        .create_report(
            CreateReportRequest {
                researcher_id: "db-cost-alice".to_string(),
                severity: Severity::Critical,
                affected_component: "db-cost-api".to_string(),
                vulnerability_type: "db-cost-rce".to_string(),
                title: "Cost test".to_string(),
                description: "Cost report".to_string(),
                proof_of_concept: None,
                submission_content: serde_json::json!({}),
            },
            admin_id,
        )
        .await
        .expect("create report");

    svc.record_reward(
        report.id,
        RecordRewardRequest {
            amount_usd: Decimal::new(5000, 0),
            justification: "Critical finding".to_string(),
            escalation_justification: None,
        },
        admin_id,
    )
    .await
    .expect("record reward");

    let metrics = svc.get_metrics().await.expect("get metrics");
    assert!(
        metrics.total_rewards_paid_usd >= Decimal::new(5000, 0),
        "total rewards must include the $5,000 reward"
    );
}
