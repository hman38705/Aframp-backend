//! Payment provider integration module
//!
//! This module provides a unified interface for payment providers (Paystack, Flutterwave, M-Pesa)
//! to support fiat transactions in African markets.

#[cfg(feature = "database")]
pub mod error;

#[cfg(feature = "database")]
pub mod error_mapping;
#[cfg(feature = "database")]
pub mod factory;
#[cfg(feature = "database")]
pub mod provider;
#[cfg(feature = "database")]
pub mod providers;
#[cfg(feature = "database")]
pub mod traits;
#[cfg(feature = "database")]
pub mod types;
#[cfg(feature = "database")]
pub mod utils;
#[cfg(feature = "database")]
pub mod circuit_breaker;

#[cfg(feature = "database")]
pub use error::{PaymentError, PaymentResult};
#[cfg(feature = "database")]
pub use error_mapping::{Locale, PaymentProviderErrorMapping};
#[cfg(feature = "database")]
pub use factory::PaymentProviderFactory;
#[cfg(feature = "database")]
pub use provider::PaymentProvider;
#[cfg(feature = "database")]
pub use types::*;
