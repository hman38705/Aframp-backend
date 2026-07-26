//! Compatibility shim for the historical `chains::stellar` client API.
//!
//! `chains::stellar::client::StellarClient` was the shared Stellar client
//! type used throughout the codebase before a partial migration toward the
//! leaner `stellar::horizon::HorizonClient`. The migration commented out
//! imports of the old type across ~30 files but never replaced the type
//! itself, leaving the crate unable to compile at all. This module restores
//! a minimal `StellarClient` wrapping `stellar::horizon::HorizonClient`
//! (plus a couple of direct Horizon HTTP calls for endpoints
//! `HorizonClient` doesn't expose) so those files resolve again.
//!
//! This was reconstructed by reading call sites, without a compiler to
//! verify against — it restores the type-level contract (method names,
//! argument/return shapes) with real Horizon-backed behavior where the
//! mapping was unambiguous. Treat it as a reviewed-on-paper, not
//! compiler-verified, follow-up.
pub mod stellar;
