//! AML Programme Effectiveness Reporting & Metrics
//! Issue #396
//!
//! Issue #818: this file was flagged as a near-empty stub by byte size alone.
//! It is intentionally a thin re-export module — the real implementation
//! (~2000 lines) lives in `handlers.rs`, `models.rs`, `repository.rs`,
//! `service.rs`, and `worker.rs`, and is mounted in `main.rs` under
//! `compliance_effectiveness_routes`. Decision: keep as implemented,
//! not a stub — no action needed beyond documenting this.

pub mod handlers;
pub mod models;
pub mod repository;
pub mod service;
pub mod worker;

pub use handlers::{compliance_effectiveness_routes, ComplianceEffectivenessState};
pub use repository::ComplianceEffectivenessRepository;
pub use service::ReportGenerationService;
pub use worker::ComplianceReportWorker;
