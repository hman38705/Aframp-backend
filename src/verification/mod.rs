//! Collateral Verification Engine (Proof-of-Reserve for cNGN).
//!
//! Issue #820: this file was flagged as a near-empty stub by byte size alone.
//! It is intentionally a thin re-export module — the implementation lives in
//! `engine.rs`, `repository.rs`, `handler.rs`, and `worker.rs`. Those were
//! previously implemented but never wired into `main.rs`; they are now
//! mounted at `/api/internal/verification/*` and run on a scheduled worker.
//! Decision: expand (wire up), not remove — this is not email/phone
//! verification, it's on-chain vs fiat reserve reconciliation.
pub mod engine;
pub mod repository;
pub mod handler;
pub mod worker;

pub use engine::{VerificationEngine, VerificationResult};
