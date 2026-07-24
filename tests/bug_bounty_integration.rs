//! Integration tests for the Bug Bounty Programme subsystem.
//!
//! These tests exercise the full report lifecycle and key programme workflows
//! using in-memory mock implementations of the repository layer, so they run
//! without a live PostgreSQL database.
//!
//! Tests that require a real database are gated with `#[cfg(feature = "integration")]`
//! and follow the same pattern as `tests/pentest_integration.rs`.
//!
//! The mock-based tests (no feature gate) exercise the service-layer business
//! logic end-to-end by calling the pure functions in `duplicate`, `sla`,
//! `rewards`, `transition`, and `notifications` directly, mirroring what
//! `BugBountyService` does internally.
//!
//! Test modules live under `tests/bug_bounty/` (see `bug_bounty/mod.rs`) so
//! each workflow can be compiled and run independently, e.g.:
//!   cargo test --test bug_bounty_integration bug_bounty::lifecycle_tests::

mod bug_bounty;
