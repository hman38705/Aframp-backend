//! Recurring payment scheduling service.
//!
//! This module is the public API surface for recurring payments. It re-exports
//! the `frequency` and `notification` sub-modules, and provides the
//! [`RecurringPaymentScheduler`] service which is the single entry-point for
//! all schedule management operations (create, update, cancel, query).
//!
//! # Architecture
//!
//! ```text
//! HTTP handlers (src/api/recurring.rs)
//!        │
//!        ▼
//! RecurringPaymentScheduler   ← this module
//!        │
//!        ├── RecurringPaymentRepository  (src/database/recurring_payment_repository.rs)
//!        │       └── PostgreSQL (recurring_payment_schedules, recurring_payment_executions)
//!        │
//!        └── RecurringPaymentWorker      (src/workers/recurring_payment_worker.rs)
//!                └── polls DB every 60 s, dispatches due schedules
//! ```
//!
//! The worker is launched independently via `main.rs` / `startup.rs`; the
//! scheduler service is used directly by HTTP handlers for CRUD operations.

pub mod frequency;
pub mod notification;

use std::sync::Arc;

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::database::recurring_payment_repository::{
    RecurringExecution, RecurringPaymentRepository, RecurringSchedule,
};
use crate::recurring::frequency::{next_execution_from_now, Frequency};

// ---------------------------------------------------------------------------
// Public re-exports for callers that only import from `crate::recurring`
// ---------------------------------------------------------------------------

pub use crate::database::recurring_payment_repository::{RecurringExecution as Execution, RecurringSchedule as Schedule};
pub use frequency::Frequency as RecurringFrequency;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can arise in the recurring payment scheduler.
#[derive(Debug, thiserror::Error)]
pub enum RecurringError {
    #[error("database error: {0}")]
    Database(#[from] crate::database::error::DatabaseError),

    #[error("schedule not found: {0}")]
    NotFound(Uuid),

    #[error("invalid frequency: {0}")]
    InvalidFrequency(String),

    #[error("invalid amount: {0}")]
    InvalidAmount(String),

    #[error("invalid transaction type '{0}'; must be bill_payment, onramp, or offramp")]
    InvalidTransactionType(String),

    #[error("invalid status transition from '{from}' to '{to}'")]
    InvalidStatusTransition { from: String, to: String },

    #[error("cannot modify a cancelled schedule")]
    ScheduleCancelled,
}

pub type RecurringResult<T> = Result<T, RecurringError>;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// All data needed to create a new recurring schedule.
#[derive(Debug, Clone)]
pub struct CreateScheduleInput {
    pub wallet_address: String,
    pub transaction_type: String,
    pub provider: Option<String>,
    /// Positive decimal string, e.g. `"5000.00"`.
    pub amount: BigDecimal,
    pub currency: String,
    pub frequency: String,
    /// Required when `frequency == "custom"`.
    pub custom_interval_days: Option<i32>,
    /// Defaults to `Utc::now()` when `None`.
    pub start_at: Option<DateTime<Utc>>,
    /// Provider-specific metadata (meter number, account number, etc.).
    pub payment_metadata: serde_json::Value,
    /// Consecutive-failure count before auto-suspension (default: 3).
    pub failure_threshold: Option<i32>,
}

/// Fields that may be changed on an existing schedule.
/// `None` means "keep the current value".
#[derive(Debug, Clone, Default)]
pub struct UpdateScheduleInput {
    pub amount: Option<BigDecimal>,
    pub frequency: Option<String>,
    pub custom_interval_days: Option<i32>,
    pub next_execution_at: Option<DateTime<Utc>>,
    /// `"paused"` or `"active"` only — use `cancel_schedule` for cancellation.
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Service layer for recurring payment schedules.
///
/// All business-rule validation lives here so that handlers and the worker
/// share a single validated path into the database.
#[derive(Clone)]
pub struct RecurringPaymentScheduler {
    repo: Arc<RecurringPaymentRepository>,
    /// Default consecutive-failure threshold used when the caller does not
    /// provide one in [`CreateScheduleInput::failure_threshold`].
    default_failure_threshold: i32,
}

impl RecurringPaymentScheduler {
    /// Construct a scheduler backed by the given repository.
    pub fn new(repo: Arc<RecurringPaymentRepository>) -> Self {
        let default_failure_threshold = std::env::var("RECURRING_FAILURE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        Self {
            repo,
            default_failure_threshold,
        }
    }

    /// Build a scheduler from a raw [`sqlx::PgPool`].
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self::new(Arc::new(RecurringPaymentRepository::new(pool)))
    }

    // -----------------------------------------------------------------------
    // Create
    // -----------------------------------------------------------------------

    /// Create and persist a new recurring payment schedule.
    ///
    /// Validates frequency, amount, and transaction type before writing to the
    /// database. Computes the first `next_execution_at` from `start_at`.
    #[instrument(skip(self, input), fields(wallet = %input.wallet_address, freq = %input.frequency))]
    pub async fn create_schedule(
        &self,
        input: CreateScheduleInput,
    ) -> RecurringResult<RecurringSchedule> {
        // --- Validate frequency ---
        let freq = Frequency::parse(&input.frequency, input.custom_interval_days)
            .map_err(RecurringError::InvalidFrequency)?;

        // --- Validate amount ---
        if input.amount <= BigDecimal::from(0) {
            return Err(RecurringError::InvalidAmount(
                "amount must be greater than zero".to_string(),
            ));
        }

        // --- Validate transaction type ---
        Self::validate_transaction_type(&input.transaction_type)?;

        let threshold = input
            .failure_threshold
            .unwrap_or(self.default_failure_threshold);

        let start = input.start_at.unwrap_or_else(Utc::now);
        let next_execution_at = next_execution_from_now(&freq, start);

        let schedule = self
            .repo
            .create_schedule(
                &input.wallet_address,
                &input.transaction_type,
                input.provider.as_deref(),
                input.amount,
                &input.currency,
                &input.frequency,
                input.custom_interval_days,
                input.payment_metadata,
                threshold,
                next_execution_at,
            )
            .await?;

        info!(
            schedule_id = %schedule.id,
            wallet       = %schedule.wallet_address,
            frequency    = %schedule.frequency,
            next_exec    = %schedule.next_execution_at,
            "Recurring schedule created"
        );

        Ok(schedule)
    }

    // -----------------------------------------------------------------------
    // Read
    // -----------------------------------------------------------------------

    /// Return a schedule, enforcing wallet ownership.
    pub async fn get_schedule(
        &self,
        id: Uuid,
        wallet_address: &str,
    ) -> RecurringResult<Option<RecurringSchedule>> {
        Ok(self.repo.find_by_id_and_wallet(id, wallet_address).await?)
    }

    /// List all schedules for a wallet, with optional status and type filters.
    pub async fn list_schedules(
        &self,
        wallet_address: &str,
        status: Option<&str>,
        transaction_type: Option<&str>,
    ) -> RecurringResult<Vec<RecurringSchedule>> {
        Ok(self
            .repo
            .list_for_wallet(wallet_address, status, transaction_type)
            .await?)
    }

    /// Return the execution history for a schedule.
    pub async fn list_executions(
        &self,
        schedule_id: Uuid,
    ) -> RecurringResult<Vec<RecurringExecution>> {
        Ok(self.repo.list_executions_for_schedule(schedule_id).await?)
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    /// Update mutable fields on an existing schedule.
    ///
    /// Validates ownership, status transitions, and — if a new frequency is
    /// provided — that it is well-formed.
    #[instrument(skip(self, input), fields(schedule_id = %id))]
    pub async fn update_schedule(
        &self,
        id: Uuid,
        wallet_address: &str,
        input: UpdateScheduleInput,
    ) -> RecurringResult<RecurringSchedule> {
        // --- Ownership & existence check ---
        let existing = self
            .repo
            .find_by_id_and_wallet(id, wallet_address)
            .await?
            .ok_or(RecurringError::NotFound(id))?;

        // --- Cannot touch a cancelled schedule ---
        if existing.status == "cancelled" {
            return Err(RecurringError::ScheduleCancelled);
        }

        // --- Validate status transition ---
        if let Some(ref new_status) = input.status {
            Self::validate_status_transition(&existing.status, new_status)?;
        }

        // --- Validate new frequency (if provided) ---
        if let Some(ref freq_str) = input.frequency {
            Frequency::parse(freq_str, input.custom_interval_days)
                .map_err(RecurringError::InvalidFrequency)?;
        }

        // --- Validate new amount (if provided) ---
        if let Some(ref amt) = input.amount {
            if *amt <= BigDecimal::from(0) {
                return Err(RecurringError::InvalidAmount(
                    "amount must be greater than zero".to_string(),
                ));
            }
        }

        let updated = self
            .repo
            .update_schedule(
                id,
                input.amount,
                input.frequency.as_deref(),
                input.custom_interval_days,
                input.next_execution_at,
                input.status.as_deref(),
            )
            .await?;

        info!(schedule_id = %id, "Recurring schedule updated");

        Ok(updated)
    }

    // -----------------------------------------------------------------------
    // Cancel
    // -----------------------------------------------------------------------

    /// Soft-cancel a schedule (sets status = `cancelled`, never deleted).
    ///
    /// Idempotent — cancelling an already-cancelled schedule is a no-op that
    /// returns the existing record.
    #[instrument(skip(self), fields(schedule_id = %id))]
    pub async fn cancel_schedule(
        &self,
        id: Uuid,
        wallet_address: &str,
    ) -> RecurringResult<RecurringSchedule> {
        // --- Ownership check ---
        let existing = self
            .repo
            .find_by_id_and_wallet(id, wallet_address)
            .await?
            .ok_or(RecurringError::NotFound(id))?;

        // Idempotent
        if existing.status == "cancelled" {
            return Ok(existing);
        }

        let cancelled = self.repo.cancel_schedule(id).await?;

        info!(schedule_id = %id, "Recurring schedule cancelled");

        Ok(cancelled)
    }

    // -----------------------------------------------------------------------
    // Worker helpers (called by RecurringPaymentWorker)
    // -----------------------------------------------------------------------

    /// Fetch all active schedules whose `next_execution_at <= now`.
    ///
    /// This is a thin pass-through to the repository; the worker holds its own
    /// `Arc<RecurringPaymentRepository>` and calls this directly, but the
    /// method is exposed here for testability.
    pub async fn fetch_due_schedules(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> RecurringResult<Vec<RecurringSchedule>> {
        Ok(self.repo.fetch_due_schedules(now, limit).await?)
    }

    /// Record a successful execution and advance the schedule.
    ///
    /// Returns `None` when the idempotency key already exists (execution was
    /// already recorded — the caller should skip advancing the schedule).
    pub async fn record_success(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
        transaction_id: Uuid,
        next_execution_at: DateTime<Utc>,
        wallet_address: &str,
        amount: &str,
        currency: &str,
    ) -> RecurringResult<Option<RecurringExecution>> {
        let execution = self
            .repo
            .insert_execution(
                schedule_id,
                scheduled_at,
                "success",
                Some(transaction_id),
                None,
            )
            .await?;

        if execution.is_none() {
            // Already recorded — idempotency guard fired.
            info!(
                schedule_id = %schedule_id,
                scheduled_at = %scheduled_at,
                "Skipping already-executed schedule (idempotency)"
            );
            return Ok(None);
        }

        self.repo
            .record_success(schedule_id, next_execution_at)
            .await?;

        notification::notify_success(schedule_id, wallet_address, transaction_id, amount, currency);

        Ok(execution)
    }

    /// Record a failed execution, increment the failure counter, and
    /// potentially auto-suspend the schedule.
    ///
    /// Returns `None` when the idempotency key already exists.
    pub async fn record_failure(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
        next_execution_at: DateTime<Utc>,
        wallet_address: &str,
        reason: &str,
    ) -> RecurringResult<Option<RecurringSchedule>> {
        let execution = self
            .repo
            .insert_execution(schedule_id, scheduled_at, "failed", None, Some(reason))
            .await?;

        if execution.is_none() {
            info!(
                schedule_id = %schedule_id,
                scheduled_at = %scheduled_at,
                "Skipping already-recorded failure (idempotency)"
            );
            return Ok(None);
        }

        let updated = self
            .repo
            .record_failure(schedule_id, next_execution_at)
            .await?;

        notification::notify_failure(
            schedule_id,
            wallet_address,
            updated.failure_count,
            reason,
        );

        if updated.status == "suspended" {
            warn!(
                schedule_id = %schedule_id,
                failure_count = updated.failure_count,
                "Schedule auto-suspended after consecutive failures"
            );
            notification::notify_suspended(schedule_id, wallet_address, updated.failure_count);
        }

        Ok(Some(updated))
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn validate_transaction_type(t: &str) -> RecurringResult<()> {
        match t {
            "bill_payment" | "onramp" | "offramp" => Ok(()),
            other => Err(RecurringError::InvalidTransactionType(other.to_string())),
        }
    }

    fn validate_status_transition(from: &str, to: &str) -> RecurringResult<()> {
        match (from, to) {
            ("active", "paused") => Ok(()),
            ("paused", "active") => Ok(()),
            // Suspended schedules can be manually reactivated by ops.
            ("suspended", "active") => Ok(()),
            _ => Err(RecurringError::InvalidStatusTransition {
                from: from.to_string(),
                to: to.to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_transaction_type ---

    #[test]
    fn valid_transaction_types_pass() {
        assert!(RecurringPaymentScheduler::validate_transaction_type("bill_payment").is_ok());
        assert!(RecurringPaymentScheduler::validate_transaction_type("onramp").is_ok());
        assert!(RecurringPaymentScheduler::validate_transaction_type("offramp").is_ok());
    }

    #[test]
    fn invalid_transaction_type_rejected() {
        let err = RecurringPaymentScheduler::validate_transaction_type("transfer");
        assert!(matches!(err, Err(RecurringError::InvalidTransactionType(_))));
    }

    // --- validate_status_transition ---

    #[test]
    fn active_to_paused_allowed() {
        assert!(RecurringPaymentScheduler::validate_status_transition("active", "paused").is_ok());
    }

    #[test]
    fn paused_to_active_allowed() {
        assert!(RecurringPaymentScheduler::validate_status_transition("paused", "active").is_ok());
    }

    #[test]
    fn suspended_to_active_allowed() {
        assert!(
            RecurringPaymentScheduler::validate_status_transition("suspended", "active").is_ok()
        );
    }

    #[test]
    fn cancelled_to_active_rejected() {
        let err =
            RecurringPaymentScheduler::validate_status_transition("cancelled", "active");
        assert!(matches!(
            err,
            Err(RecurringError::InvalidStatusTransition { .. })
        ));
    }

    #[test]
    fn active_to_cancelled_rejected_via_transition_validator() {
        // Cancellation goes through cancel_schedule(), not update_schedule().
        let err =
            RecurringPaymentScheduler::validate_status_transition("active", "cancelled");
        assert!(matches!(
            err,
            Err(RecurringError::InvalidStatusTransition { .. })
        ));
    }

    #[test]
    fn active_to_suspended_rejected_via_transition_validator() {
        // Suspension is automatic (worker), not user-driven.
        let err =
            RecurringPaymentScheduler::validate_status_transition("active", "suspended");
        assert!(matches!(
            err,
            Err(RecurringError::InvalidStatusTransition { .. })
        ));
    }

    // --- amount validation ---

    #[test]
    fn zero_amount_error_message() {
        let err = RecurringError::InvalidAmount("amount must be greater than zero".to_string());
        assert!(err.to_string().contains("amount"));
    }

    // --- frequency round-trips ---

    #[test]
    fn frequency_parse_daily() {
        assert!(Frequency::parse("daily", None).is_ok());
    }

    #[test]
    fn frequency_parse_custom_requires_days() {
        assert!(Frequency::parse("custom", None).is_err());
        assert!(Frequency::parse("custom", Some(7)).is_ok());
    }

    // --- error display ---

    #[test]
    fn recurring_error_display_not_found() {
        let id = Uuid::nil();
        let err = RecurringError::NotFound(id);
        assert!(err.to_string().contains(&id.to_string()));
    }

    #[test]
    fn recurring_error_display_cancelled() {
        let err = RecurringError::ScheduleCancelled;
        assert!(err.to_string().contains("cancelled"));
    }
}
