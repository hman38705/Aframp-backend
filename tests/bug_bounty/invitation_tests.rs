//! 14.3 Invitation management integration test

use super::helpers::*;
use std::sync::Arc;
use uuid::Uuid;

/// Requirements: 13.3
///
/// Verifies the private programme invitation management workflow:
///   - Creating an invitation for a researcher.
///   - Accepting a report from an invited researcher.
///   - Rejecting a report from an uninvited researcher with InvitationRequired.
#[tokio::test]
async fn invitation_management_workflow() {
    let config = BugBountyConfig::default();
    let admin_id = Uuid::new_v4();

    // Programme is in private phase
    let store = MockStore::new(ProgrammePhase::Private);
    let dispatcher = NotificationDispatcher::new(Arc::clone(&store));

    // ── Create invitation for "alice" ────────────────────────────────────
    let invitation = create_invitation(&store, "alice", admin_id).await;
    assert_eq!(invitation.researcher_id, "alice");
    assert_eq!(invitation.status, "active");

    let invitations = store.invitations.lock().await.clone();
    assert_eq!(invitations.len(), 1);
    drop(invitations);

    // ── Alice (invited) can submit a report ──────────────────────────────
    let req_alice = make_report_request("alice", "api/payments", "idor", Severity::High);
    let report_alice = create_report(&store, &dispatcher, req_alice, &config)
        .await
        .expect("alice (invited) should be able to submit a report");

    assert_eq!(report_alice.status, ReportStatus::New);
    assert_eq!(report_alice.researcher_id, "alice");

    // ── Bob (not invited) is rejected ────────────────────────────────────
    let req_bob = make_report_request("bob", "api/payments", "sqli", Severity::Critical);
    let result_bob = create_report(&store, &dispatcher, req_bob, &config).await;

    assert!(
        result_bob.is_err(),
        "bob (uninvited) must be rejected during private phase"
    );
    assert!(
        matches!(
            result_bob.unwrap_err(),
            Bitmesh_backend::bug_bounty::models::BugBountyError::InvitationRequired
        ),
        "error must be InvitationRequired"
    );

    // Only alice's report is in the store
    let reports = store.reports.lock().await.clone();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].researcher_id, "alice");
}
