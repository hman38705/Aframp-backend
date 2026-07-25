//! Shared mock infrastructure and fixtures used by the bug_bounty test modules.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub use Bitmesh_backend::bug_bounty::models::{
    BugBountyConfig, BugBountyReport, CommunicationLogEntry, CreateInvitationRequest,
    CreateReportRequest, ProgrammePhase, ProgrammeState, RecordRewardRequest, ReportStatus,
    ResearcherInvitation, RewardRecord, Severity, TransitionResult, UnmetCriterion,
    UpdateReportRequest,
};
pub use Bitmesh_backend::bug_bounty::notifications::{
    disclosure_date_after_resolution, NotificationDispatcher, NotificationRepository,
};
pub use Bitmesh_backend::bug_bounty::transition::ProgrammeStats;
pub use Bitmesh_backend::bug_bounty::{duplicate, notifications, rewards, sla, transition};

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

// -----------------------------------------------------------------------
// In-memory mock repository
// -----------------------------------------------------------------------

/// Thread-safe in-memory store used by all integration tests.
pub struct MockStore {
    pub reports: Mutex<Vec<BugBountyReport>>,
    pub comm_log: Mutex<Vec<CommunicationLogEntry>>,
    pub rewards: Mutex<Vec<RewardRecord>>,
    pub invitations: Mutex<Vec<ResearcherInvitation>>,
    pub programme_state: Mutex<ProgrammeState>,
}

impl MockStore {
    fn make_state(phase: ProgrammePhase, launched_at: DateTime<Utc>) -> ProgrammeState {
        ProgrammeState {
            id: Uuid::new_v4(),
            phase,
            launched_at,
            transitioned_to_public_at: None,
            transitioned_by: None,
        }
    }

    pub fn new(phase: ProgrammePhase) -> Arc<Self> {
        Arc::new(Self {
            reports: Mutex::new(Vec::new()),
            comm_log: Mutex::new(Vec::new()),
            rewards: Mutex::new(Vec::new()),
            invitations: Mutex::new(Vec::new()),
            programme_state: Mutex::new(Self::make_state(
                phase,
                Utc::now() - Duration::days(31),
            )),
        })
    }

    pub fn new_with_launch(phase: ProgrammePhase, launched_at: DateTime<Utc>) -> Arc<Self> {
        Arc::new(Self {
            reports: Mutex::new(Vec::new()),
            comm_log: Mutex::new(Vec::new()),
            rewards: Mutex::new(Vec::new()),
            invitations: Mutex::new(Vec::new()),
            programme_state: Mutex::new(Self::make_state(phase, launched_at)),
        })
    }
}

#[async_trait]
impl NotificationRepository for MockStore {
    async fn insert_communication_log_entry(
        &self,
        entry: &CommunicationLogEntry,
    ) -> Result<(), Bitmesh_backend::bug_bounty::models::BugBountyError> {
        self.comm_log.lock().await.push(entry.clone());
        Ok(())
    }
}

// -----------------------------------------------------------------------
// Service-layer helpers (replicate BugBountyService logic inline)
// -----------------------------------------------------------------------

/// Create a report, run duplicate detection, compute SLA deadlines,
/// persist to the mock store, and dispatch an acknowledgement notification.
pub async fn create_report(
    store: &Arc<MockStore>,
    dispatcher: &NotificationDispatcher<MockStore>,
    req: CreateReportRequest,
    config: &BugBountyConfig,
) -> Result<BugBountyReport, Bitmesh_backend::bug_bounty::models::BugBountyError> {
    // Private-phase invitation check
    let phase = store.programme_state.lock().await.phase.clone();
    if phase == ProgrammePhase::Private {
        let invitations = store.invitations.lock().await;
        let has_valid = invitations
            .iter()
            .any(|i| i.researcher_id == req.researcher_id && i.status == "active");
        if !has_valid {
            return Err(
                Bitmesh_backend::bug_bounty::models::BugBountyError::InvitationRequired,
            );
        }
    }

    // Duplicate detection
    let open_reports: Vec<BugBountyReport> = store
        .reports
        .lock()
        .await
        .iter()
        .filter(|r| {
            !matches!(
                r.status,
                ReportStatus::Duplicate
                    | ReportStatus::OutOfScope
                    | ReportStatus::Rejected
                    | ReportStatus::Resolved
            )
        })
        .cloned()
        .collect();

    let original_id = duplicate::find_original(&req, &open_reports);
    let is_duplicate = original_id.is_some();
    let status = if is_duplicate {
        ReportStatus::Duplicate
    } else {
        ReportStatus::New
    };

    // SLA deadlines
    let now = Utc::now();
    let (ack_deadline, triage_deadline) = sla::compute_deadlines(now, config);

    let report = BugBountyReport {
        id: Uuid::new_v4(),
        researcher_id: req.researcher_id.clone(),
        severity: req.severity.clone(),
        affected_component: req.affected_component.clone(),
        vulnerability_type: req.vulnerability_type.clone(),
        title: req.title.clone(),
        description: req.description.clone(),
        proof_of_concept: req.proof_of_concept.clone(),
        submission_content: req.submission_content.clone(),
        status,
        duplicate_of: original_id,
        acknowledgement_sla_deadline: ack_deadline,
        triage_sla_deadline: triage_deadline,
        acknowledged_at: None,
        triaged_at: None,
        resolved_at: None,
        coordinated_disclosure_date: None,
        remediation_ref: None,
        source: "managed_platform".to_string(),
        created_at: now,
        updated_at: now,
    };

    store.reports.lock().await.push(report.clone());
    dispatcher.send_acknowledgement(&report).await?;

    Ok(report)
}

/// Update a report's status in the mock store, setting timestamp fields
/// automatically (mirrors BugBountyService::update_report).
pub async fn update_report_status(
    store: &Arc<MockStore>,
    dispatcher: &NotificationDispatcher<MockStore>,
    report_id: Uuid,
    new_status: ReportStatus,
) -> Result<BugBountyReport, Bitmesh_backend::bug_bounty::models::BugBountyError> {
    let now = Utc::now();
    let mut reports = store.reports.lock().await;
    let report = reports
        .iter_mut()
        .find(|r| r.id == report_id)
        .ok_or(Bitmesh_backend::bug_bounty::models::BugBountyError::ReportNotFound)?;

    match &new_status {
        ReportStatus::Acknowledged if report.acknowledged_at.is_none() => {
            report.acknowledged_at = Some(now);
        }
        ReportStatus::Triaged if report.triaged_at.is_none() => {
            report.triaged_at = Some(now);
        }
        ReportStatus::Resolved if report.resolved_at.is_none() => {
            report.resolved_at = Some(now);
            report.coordinated_disclosure_date = Some(disclosure_date_after_resolution(now));
        }
        _ => {}
    }

    report.status = new_status.clone();
    report.updated_at = now;
    let updated = report.clone();
    drop(reports);

    match &new_status {
        ReportStatus::Resolved => {
            let disclosure_date = updated
                .coordinated_disclosure_date
                .unwrap_or_else(|| disclosure_date_after_resolution(now));
            dispatcher
                .send_coordinated_disclosure(&updated, disclosure_date)
                .await?;
        }
        _ => {
            dispatcher.send_status_update(&updated).await?;
        }
    }

    Ok(updated)
}

/// Record a reward for a report in the mock store.
pub async fn record_reward(
    store: &Arc<MockStore>,
    dispatcher: &NotificationDispatcher<MockStore>,
    report_id: Uuid,
    req: RecordRewardRequest,
    admin_id: Uuid,
    config: &BugBountyConfig,
) -> Result<RewardRecord, Bitmesh_backend::bug_bounty::models::BugBountyError> {
    let report = {
        let reports = store.reports.lock().await;
        reports
            .iter()
            .find(|r| r.id == report_id)
            .cloned()
            .ok_or(Bitmesh_backend::bug_bounty::models::BugBountyError::ReportNotFound)?
    };

    rewards::validate_tier(
        req.amount_usd,
        &report.severity,
        config,
        req.escalation_justification.as_deref(),
    )?;

    let now = Utc::now();
    let reward = RewardRecord {
        id: Uuid::new_v4(),
        report_id,
        researcher_id: report.researcher_id.clone(),
        amount_usd: req.amount_usd,
        justification: req.justification.clone(),
        escalation_justification: req.escalation_justification.clone(),
        payment_initiated_at: now,
        created_by: admin_id,
        created_at: now,
    };

    store.rewards.lock().await.push(reward.clone());
    dispatcher.send_reward_decision(&report, &reward).await?;

    Ok(reward)
}

/// Create an invitation in the mock store.
pub async fn create_invitation(
    store: &Arc<MockStore>,
    researcher_id: &str,
    admin_id: Uuid,
) -> ResearcherInvitation {
    let now = Utc::now();
    let invitation = ResearcherInvitation {
        id: Uuid::new_v4(),
        researcher_id: researcher_id.to_string(),
        status: "active".to_string(),
        created_by: admin_id,
        created_at: now,
        revoked_at: None,
        revoked_by: None,
    };
    store.invitations.lock().await.push(invitation.clone());
    invitation
}

/// Attempt a private-to-public transition using the transition evaluator.
pub async fn attempt_transition(
    store: &Arc<MockStore>,
    config: &BugBountyConfig,
    admin_id: Uuid,
) -> Result<TransitionResult, Bitmesh_backend::bug_bounty::models::BugBountyError> {
    let state = store.programme_state.lock().await.clone();
    let reports = store.reports.lock().await.clone();

    let researchers_participated: u32 = {
        let ids: std::collections::HashSet<&str> =
            reports.iter().map(|r| r.researcher_id.as_str()).collect();
        ids.len() as u32
    };

    let valid_statuses = [
        ReportStatus::Acknowledged,
        ReportStatus::Triaged,
        ReportStatus::InRemediation,
        ReportStatus::Resolved,
    ];
    let valid_findings_processed = reports
        .iter()
        .filter(|r| valid_statuses.contains(&r.status))
        .count() as u32;

    let resolved_count = reports
        .iter()
        .filter(|r| r.status == ReportStatus::Resolved)
        .count();
    let remediation_rate_percent = if valid_findings_processed == 0 {
        0.0
    } else {
        (resolved_count as f64 / valid_findings_processed as f64) * 100.0
    };

    let stats = ProgrammeStats {
        researchers_participated,
        valid_findings_processed,
        remediation_rate_percent,
    };

    let result = transition::evaluate_criteria(&state, &stats, config);

    if result.success {
        let now = Utc::now();
        let mut ps = store.programme_state.lock().await;
        ps.phase = ProgrammePhase::Public;
        ps.transitioned_to_public_at = Some(now);
        ps.transitioned_by = Some(admin_id);
        Ok(result)
    } else {
        Err(
            Bitmesh_backend::bug_bounty::models::BugBountyError::TransitionCriteriaNotMet {
                unmet: result.unmet_criteria,
            },
        )
    }
}

// -----------------------------------------------------------------------
// Shared request builders
// -----------------------------------------------------------------------

pub fn make_config_easy_transition() -> BugBountyConfig {
    BugBountyConfig {
        min_invited_researchers_participated: 1,
        min_valid_findings_processed: 1,
        min_remediation_rate_percent: 0.0,
        stabilisation_period_days: 0,
        ..BugBountyConfig::default()
    }
}

pub fn make_report_request(
    researcher_id: &str,
    component: &str,
    vuln_type: &str,
    severity: Severity,
) -> CreateReportRequest {
    CreateReportRequest {
        researcher_id: researcher_id.to_string(),
        severity,
        affected_component: component.to_string(),
        vulnerability_type: vuln_type.to_string(),
        title: format!("{vuln_type} in {component}"),
        description: "Integration test report".to_string(),
        proof_of_concept: Some("PoC details".to_string()),
        submission_content: json!({"source": "integration_test"}),
    }
}
