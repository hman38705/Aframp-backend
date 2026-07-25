//! 14.2 Duplicate detection workflow integration test

use super::helpers::*;
use std::sync::Arc;

/// Requirements: 13.2
///
/// Submits multiple reports with overlapping affected_component and
/// vulnerability_type. Verifies that:
///   - The second report (same component + same vuln_type) is flagged as Duplicate
///     and records the original report's ID.
///   - The original report is unaffected (still New).
///   - A third report with the same component but a different vuln_type is NOT
///     flagged as a duplicate (status = New).
#[tokio::test]
async fn duplicate_detection_workflow() {
    let config = BugBountyConfig::default();
    let store = MockStore::new(ProgrammePhase::Public);
    let dispatcher = NotificationDispatcher::new(Arc::clone(&store));

    // ── Report A: original ───────────────────────────────────────────────
    let req_a = make_report_request("alice", "api/auth", "sqli", Severity::High);
    let report_a = create_report(&store, &dispatcher, req_a, &config)
        .await
        .expect("create report A");

    assert_eq!(report_a.status, ReportStatus::New);
    assert!(report_a.duplicate_of.is_none());

    // ── Report B: same component + same vuln_type → Duplicate ────────────
    let req_b = make_report_request("bob", "api/auth", "sqli", Severity::Critical);
    let report_b = create_report(&store, &dispatcher, req_b, &config)
        .await
        .expect("create report B");

    assert_eq!(
        report_b.status,
        ReportStatus::Duplicate,
        "report B must be flagged as Duplicate"
    );
    assert_eq!(
        report_b.duplicate_of,
        Some(report_a.id),
        "report B must reference report A as the original"
    );

    // Original report A is unaffected
    let reports = store.reports.lock().await.clone();
    let a_in_store = reports
        .iter()
        .find(|r| r.id == report_a.id)
        .expect("report A must exist in store");
    assert_eq!(
        a_in_store.status,
        ReportStatus::New,
        "original report A must remain New"
    );
    drop(reports);

    // ── Report C: same component, different vuln_type → New ──────────────
    let req_c = make_report_request("carol", "api/auth", "xss", Severity::Medium);
    let report_c = create_report(&store, &dispatcher, req_c, &config)
        .await
        .expect("create report C");

    assert_eq!(
        report_c.status,
        ReportStatus::New,
        "report C (different vuln_type) must NOT be flagged as Duplicate"
    );
    assert!(report_c.duplicate_of.is_none());

    // Communication log: A gets ack, B gets ack (duplicate flag in content),
    // C gets ack — 3 acknowledgement entries total.
    let log = store.comm_log.lock().await.clone();
    let ack_entries: Vec<_> = log
        .iter()
        .filter(|e| e.notification_type == "acknowledgement")
        .collect();
    assert_eq!(ack_entries.len(), 3, "expected 3 acknowledgement entries");
}
