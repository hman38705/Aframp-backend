//! 14.1 Full report lifecycle integration test

use super::helpers::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

/// Requirements: 13.1
///
/// Exercises the full report lifecycle:
///   intake → acknowledgement → triage → reward → resolution
///
/// Verifies that `status`, `communication_log`, and `reward` records are
/// correctly persisted at each stage.
#[tokio::test]
async fn full_report_lifecycle() {
    let config = BugBountyConfig::default();
    let admin_id = Uuid::new_v4();

    // Programme is public so no invitation check is needed for this test.
    let store = MockStore::new(ProgrammePhase::Public);
    let dispatcher = NotificationDispatcher::new(Arc::clone(&store));

    // ── Stage 1: Intake ──────────────────────────────────────────────────
    let req = make_report_request("alice", "api/auth", "sqli", Severity::High);
    let report = create_report(&store, &dispatcher, req, &config)
        .await
        .expect("create_report should succeed");

    // Status is New immediately after intake
    assert_eq!(report.status, ReportStatus::New);
    assert!(report.acknowledged_at.is_none());

    // Communication log has exactly 1 entry (acknowledgement)
    let log = store.comm_log.lock().await.clone();
    assert_eq!(log.len(), 1, "expected 1 comm log entry after intake");
    assert_eq!(log[0].notification_type, "acknowledgement");
    assert_eq!(log[0].report_id, report.id);
    drop(log);

    // ── Stage 2: Acknowledged ────────────────────────────────────────────
    let acked =
        update_report_status(&store, &dispatcher, report.id, ReportStatus::Acknowledged)
            .await
            .expect("update to Acknowledged should succeed");

    assert_eq!(acked.status, ReportStatus::Acknowledged);
    assert!(
        acked.acknowledged_at.is_some(),
        "acknowledged_at must be set"
    );

    let log = store.comm_log.lock().await.clone();
    assert_eq!(
        log.len(),
        2,
        "expected 2 comm log entries after acknowledgement"
    );
    assert_eq!(log[1].notification_type, "status_update");
    drop(log);

    // ── Stage 3: Triaged ─────────────────────────────────────────────────
    let triaged = update_report_status(&store, &dispatcher, report.id, ReportStatus::Triaged)
        .await
        .expect("update to Triaged should succeed");

    assert_eq!(triaged.status, ReportStatus::Triaged);
    assert!(triaged.triaged_at.is_some(), "triaged_at must be set");

    let log = store.comm_log.lock().await.clone();
    assert_eq!(log.len(), 3, "expected 3 comm log entries after triage");
    assert_eq!(log[2].notification_type, "status_update");
    drop(log);

    // ── Stage 4: Reward ──────────────────────────────────────────────────
    let reward_req = RecordRewardRequest {
        amount_usd: Decimal::new(2000, 0), // $2,000 — within High tier ($1k–$5k)
        justification: "Valid high-severity finding".to_string(),
        escalation_justification: None,
    };
    let reward = record_reward(
        &store,
        &dispatcher,
        report.id,
        reward_req,
        admin_id,
        &config,
    )
    .await
    .expect("record_reward should succeed");

    assert_eq!(reward.report_id, report.id);
    assert_eq!(reward.amount_usd, Decimal::new(2000, 0));

    let rewards = store.rewards.lock().await.clone();
    assert_eq!(rewards.len(), 1, "expected 1 reward record");
    assert_eq!(rewards[0].id, reward.id);
    drop(rewards);

    let log = store.comm_log.lock().await.clone();
    assert_eq!(log.len(), 4, "expected 4 comm log entries after reward");
    assert_eq!(log[3].notification_type, "reward_decision");
    drop(log);

    // ── Stage 5: Resolved ────────────────────────────────────────────────
    let resolved = update_report_status(&store, &dispatcher, report.id, ReportStatus::Resolved)
        .await
        .expect("update to Resolved should succeed");

    assert_eq!(resolved.status, ReportStatus::Resolved);
    assert!(resolved.resolved_at.is_some(), "resolved_at must be set");
    assert!(
        resolved.coordinated_disclosure_date.is_some(),
        "coordinated_disclosure_date must be set on resolution"
    );
    // Disclosure date must be strictly after resolved_at
    let disclosure_date = resolved
        .coordinated_disclosure_date
        .expect("coordinated_disclosure_date must be set");
    let resolved_at = resolved
        .resolved_at
        .expect("resolved_at must be set");
    assert!(
        disclosure_date > resolved_at,
        "coordinated_disclosure_date must be after resolved_at"
    );

    let log = store.comm_log.lock().await.clone();
    assert_eq!(log.len(), 5, "expected 5 comm log entries after resolution");
    assert_eq!(log[4].notification_type, "coordinated_disclosure");
}
