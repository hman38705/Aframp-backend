//! Middleware modules for Aframp backend

#[cfg(feature = "database")]
pub mod api_key;

#[cfg(feature = "database")]
pub mod error;

#[cfg(feature = "database")]
pub mod geo_restriction;

#[cfg(feature = "database")]
pub mod hmac_signing;

#[cfg(feature = "database")]
pub mod ip_blocking;

#[cfg(feature = "database")]
pub mod logging;

pub mod metrics;

#[cfg(feature = "database")]
pub mod rate_limit;

pub mod rate_limit_metrics;

#[cfg(feature = "database")]
pub mod replay_prevention;

#[cfg(feature = "database")]
pub mod rbac;

#[cfg(feature = "database")]
pub mod request_integrity;

#[cfg(feature = "database")]
pub mod scope_middleware;

pub mod cors;
pub mod csrf;
pub mod security;
#[cfg(feature = "database")]
pub mod auth_rate_limit;
#[cfg(feature = "database")]
pub mod paystack_ip_allowlist;

#[cfg(feature = "database")]
pub use auth_rate_limit::{
    auth_rate_limit_middleware, record_auth_failure, record_auth_success, AuthRateLimitConfig,
    AuthRateLimitState,
};
