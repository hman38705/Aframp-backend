//! 14.4 Transition workflow and 14.5 monthly cost report integration tests

use super::helpers::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

/// Requirements: 13.4
///
/// Verifies the private-to-public transition workflow:
///   1. Attempt transition with no findings → fails with TransitionCriteriaNotMet,
///      unmet_criteria is non-empty.
///   2. Create enough reports and invitations to meet all criteria.
///   3. Attempt transition → succeeds.
///   4. After transition, an uninvited researcher can submit a report.
#[tokio::test]
async fn transition_workflow() {
    // Use a config with low thresholds so we can satisfy them easily.
    let config = BugBountyConfig {
        min_invited_researchers_participated: 2,
        min_valid_findings_processed: 2,
        min_remediation_rate_percent: 100.0,
        stabilisation_period_days: 0, // no wait required
        ..BugBountyConfig::default()
    };
    let admin_id = Uuid::new_v4();

    // Programme launched 1 day ago (stabilisation_period_days = 0 so this is fine)
    let store = MockStore::new_with_launch(
        ProgrammePhase::Private,
        chrono::Utc::now() - chrono::Duration::days(1),
    );
    let dispatcher = NotificationDispatcher::new(Arc::clone(&store));

    // ── Step 1: Attempt transition with no findings → must fail ──────────
    let result = attempt_transition(&store, &config, admin_id).await;
    assert!(
        result.is_err(),
        "transition must fail when criteria are unmet"
    );
    if let Err(
        Bitmesh_backend::bug_bounty::models::BugBountyError::TransitionCriteriaNotMet { unmet },
    ) = result
    {
        assert!(
            !unmet.is_empty(),
            "unmet_criteria must be non-empty when transition fails"
        );
    } else {
        assert!(false, "expected TransitionCriteriaNotMet error, got a different result");
    }

    // ── Step 2: Satisfy all criteria ─────────────────────────────────────
    // Invite two researchers
    create_invitation(&store, "alice", admin_id).await;
    create_invitation(&store, "bob", admin_id).await;

    // Alice submits a report
    let req_alice = make_report_request("alice", "api/auth", "sqli", Severity::High);
    let report_alice = create_report(&store, &dispatcher, req_alice, &config)
        .await
        .expect("alice report");

    // Bob submits a report
    let req_bob = make_report_request("bob", "api/payments", "idor", Severity::Medium);
    let report_bob = create_report(&store, &dispatcher, req_bob, &config)
        .await
        .expect("bob report");

    // Move both reports to Resolved (satisfies remediation_rate = 100%)
    update_report_status(&store, &dispatcher, report_alice.id, ReportStatus::Triaged)
        .await
        .expect("triaging alice's report should succeed");
    update_report_status(&store, &dispatcher, report_alice.id, ReportStatus::Resolved)
        .await
        .expect("resolving alice's report should succeed");
    update_report_status(&store, &dispatcher, report_bob.id, ReportStatus::Triaged)
        .await
        .expect("triaging bob's report should succeed");
    update_report_status(&store, &dispatcher, report_bob.id, ReportStatus::Resolved)
        .await
        .expect("resolving bob's report should succeed");

    // ── Step 3: Attempt transition → must succeed ────────────────────────
    let result = attempt_transition(&store, &config, admin_id).await;
    assert!(
        result.is_ok(),
        "transition must succeed when all criteria are met"
    );
    let transition_result = result.expect("transition result must be Ok");
    assert!(transition_result.success);
    assert!(transition_result.unmet_criteria.is_empty());

    // Programme phase is now Public
    let phase = store.programme_state.lock().await.phase.clone();
    assert_eq!(phase, ProgrammePhase::Public);

    // ── Step 4: Uninvited researcher can submit after transition ──────────
    let req_carol = make_report_request("carol", "api/admin", "xss", Severity::Low);
    let report_carol = create_report(&store, &dispatcher, req_carol, &config)
        .await
        .expect("carol (uninvited) must be able to submit after public transition");

    assert_eq!(report_carol.researcher_id, "carol");
    assert_eq!(report_carol.status, ReportStatus::New);
}

/// Requirements: 13.5
///
/// Processes a known set of reports and rewards, then verifies that the
/// aggregated totals match the expected values.
///
/// Reports:
///   - Report 1: Critical  → reward $5,000
///   - Report 2: High      → reward $2,000
///   - Report 3: Medium    → reward $500
///
/// Expected total: $7,500
#[tokio::test]
async fn monthly_cost_report() {
    let config = BugBountyConfig::default();
    let admin_id = Uuid::new_v4();

    let store = MockStore::new(ProgrammePhase::Public);
    let dispatcher = NotificationDispatcher::new(Arc::clone(&store));

    // ── Create 3 reports ─────────────────────────────────────────────────
    let req_critical = make_report_request("alice", "api/auth", "rce", Severity::Critical);
    let report_critical = create_report(&store, &dispatcher, req_critical, &config)
        .await
        .expect("critical report");

    let req_high = make_report_request("bob", "api/payments", "sqli", Severity::High);
    let report_high = create_report(&store, &dispatcher, req_high, &config)
        .await
        .expect("high report");

    let req_medium = make_report_request("carol", "api/users", "idor", Severity::Medium);
    let report_medium = create_report(&store, &dispatcher, req_medium, &config)
        .await
        .expect("medium report");

    // ── Record rewards ───────────────────────────────────────────────────
    let reward_critical = record_reward(
        &store,
        &dispatcher,
        report_critical.id,
        RecordRewardRequest {
            amount_usd: Decimal::new(5000, 0),
            justification: "Critical RCE".to_string(),
            escalation_justification: None,
        },
        admin_id,
        &config,
    )
    .await
    .expect("critical reward");

    let reward_high = record_reward(
        &store,
        &dispatcher,
        report_high.id,
        RecordRewardRequest {
            amount_usd: Decimal::new(2000, 0),
            justification: "High SQL injection".to_string(),
            escalation_justification: None,
        },
        admin_id,
        &config,
    )
    .await
    .expect("high reward");

    let reward_medium = record_reward(
        &store,
        &dispatcher,
        report_medium.id,
        RecordRewardRequest {
            amount_usd: Decimal::new(500, 0),
            justification: "Medium IDOR".to_string(),
            escalation_justification: None,
        },
        admin_id,
        &config,
    )
    .await
    .expect("medium reward");

    // ── Verify individual reward amounts ─────────────────────────────────
    assert_eq!(reward_critical.amount_usd, Decimal::new(5000, 0));
    assert_eq!(reward_high.amount_usd, Decimal::new(2000, 0));
    assert_eq!(reward_medium.amount_usd, Decimal::new(500, 0));

    // ── Verify total rewards paid = $7,500 ───────────────────────────────
    let rewards = store.rewards.lock().await.clone();
    assert_eq!(rewards.len(), 3, "expected 3 reward records");

    let total: Decimal = rewards.iter().map(|r| r.amount_usd).sum();
    assert_eq!(
        total,
        Decimal::new(7500, 0),
        "total rewards paid must equal $7,500"
    );

    // ── Verify per-researcher totals ─────────────────────────────────────
    let mut by_researcher: std::collections::HashMap<String, Decimal> =
        std::collections::HashMap::new();
    for r in &rewards {
        *by_researcher.entry(r.researcher_id.clone()).or_default() += r.amount_usd;
    }
    assert_eq!(
        by_researcher["alice"],
        Decimal::new(5000, 0),
        "alice total must be $5,000"
    );
    assert_eq!(
        by_researcher["bob"],
        Decimal::new(2000, 0),
        "bob total must be $2,000"
    );
    assert_eq!(
        by_researcher["carol"],
        Decimal::new(500, 0),
        "carol total must be $500"
    );

    // ── Verify sum_rewards_by_month equivalent ───────────────────────────
    // All rewards were created in the same calendar month (now), so the
    // monthly total must equal the grand total.
    let current_month = chrono::Utc::now().format("%Y-%m").to_string();
    let mut by_month: std::collections::HashMap<String, Decimal> =
        std::collections::HashMap::new();
    for r in &rewards {
        let month = r.created_at.format("%Y-%m").to_string();
        *by_month.entry(month).or_default() += r.amount_usd;
    }
    assert_eq!(
        by_month[&current_month],
        Decimal::new(7500, 0),
        "monthly total for current month must equal $7,500"
    );
}
