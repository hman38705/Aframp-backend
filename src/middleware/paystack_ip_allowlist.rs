//! Paystack Webhook IP Allowlist Middleware (Issue #779)
//!
//! Enforces Paystack's published source IP ranges on the webhook endpoint.
//! Requests from IPs outside the allowlist are rejected with HTTP 403
//! **before** HMAC verification runs, reducing attack surface.
//!
//! # Configuration
//! The allowlist defaults to Paystack's published CIDR ranges and can be
//! extended via the `PAYSTACK_WEBHOOK_ALLOWED_CIDRS` environment variable
//! (comma-separated, e.g. `"52.31.139.75/32,52.49.173.169/32"`).
//!
//! # Logging
//! Rejected IPs are logged at WARN level with the field
//! `event = "paystack_webhook_ip_rejected"` so they can be alerted on.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::IpAddr;
use tracing::{info, warn};

// ── Paystack published IP ranges (as of 2024) ─────────────────────────────────
// Source: https://paystack.com/docs/payments/webhooks/#ip-whitelisting

const PAYSTACK_CIDRS: &[&str] = &[
    "52.31.139.75/32",
    "52.49.173.169/32",
    "52.214.14.220/32",
];

// ── CIDR helper ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Cidr {
    network: std::net::IpAddr,
    prefix_len: u8,
}

impl Cidr {
    fn parse(s: &str) -> Option<Self> {
        let mut parts = s.splitn(2, '/');
        let addr: IpAddr = parts.next()?.parse().ok()?;
        let prefix_len: u8 = parts.next().unwrap_or("32").parse().ok()?;
        Some(Self { network: addr, prefix_len })
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(net), IpAddr::V4(candidate)) => {
                let mask = if self.prefix_len == 0 {
                    0u32
                } else {
                    u32::MAX << (32 - self.prefix_len)
                };
                (u32::from(net) & mask) == (u32::from(candidate) & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(candidate)) => {
                let mask = if self.prefix_len == 0 {
                    0u128
                } else {
                    u128::MAX << (128 - self.prefix_len)
                };
                (u128::from(net) & mask) == (u128::from(candidate) & mask)
            }
            _ => false,
        }
    }
}

// ── Allowlist ─────────────────────────────────────────────────────────────────

/// Immutable, cloneable allowlist.  Build once at startup.
#[derive(Debug, Clone)]
pub struct PaystackIpAllowlist {
    cidrs: Vec<Cidr>,
}

impl PaystackIpAllowlist {
    /// Build from the compiled-in defaults plus any `PAYSTACK_WEBHOOK_ALLOWED_CIDRS`
    /// entries found in the environment.
    pub fn from_env() -> Self {
        let mut cidrs: Vec<Cidr> = PAYSTACK_CIDRS
            .iter()
            .filter_map(|s| Cidr::parse(s))
            .collect();

        if let Ok(extra) = std::env::var("PAYSTACK_WEBHOOK_ALLOWED_CIDRS") {
            for entry in extra.split(',').map(str::trim) {
                if let Some(c) = Cidr::parse(entry) {
                    cidrs.push(c);
                }
            }
        }

        info!(cidr_count = cidrs.len(), "Paystack webhook IP allowlist initialised");
        Self { cidrs }
    }

    /// Returns `true` when `ip` matches at least one allowlisted CIDR.
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        self.cidrs.iter().any(|c| c.contains(ip))
    }
}

// ── Middleware ────────────────────────────────────────────────────────────────

/// Axum middleware layer.  Wire onto the Paystack webhook route:
///
/// ```rust,ignore
/// Router::new()
///     .route("/webhooks/paystack", post(paystack_webhook_handler))
///     .layer(axum::middleware::from_fn_with_state(
///         allowlist.clone(),
///         paystack_ip_allowlist_middleware,
///     ))
/// ```
pub async fn paystack_ip_allowlist_middleware(
    axum::extract::State(allowlist): axum::extract::State<PaystackIpAllowlist>,
    request: Request,
    next: Next,
) -> Response {
    let client_ip = extract_ip(&request);

    match client_ip {
        Some(ip) if allowlist.is_allowed(ip) => {
            info!(ip = %ip, "Paystack webhook — IP allowlisted, forwarding");
            next.run(request).await
        }
        Some(ip) => {
            warn!(
                event = "paystack_webhook_ip_rejected",
                ip = %ip,
                "Paystack webhook rejected: source IP not in allowlist"
            );
            (StatusCode::FORBIDDEN, "Forbidden: source IP not allowlisted").into_response()
        }
        None => {
            warn!(
                event = "paystack_webhook_ip_rejected",
                ip = "unknown",
                "Paystack webhook rejected: could not determine source IP"
            );
            (StatusCode::FORBIDDEN, "Forbidden: source IP indeterminate").into_response()
        }
    }
}

// ── IP extraction ─────────────────────────────────────────────────────────────

fn extract_ip(request: &Request) -> Option<IpAddr> {
    // Prefer CF-Connecting-IP (Cloudflare), then X-Real-IP, then X-Forwarded-For.
    for header_name in &["CF-Connecting-IP", "X-Real-IP", "X-Forwarded-For"] {
        if let Some(val) = request.headers().get(*header_name) {
            if let Ok(s) = val.to_str() {
                // X-Forwarded-For may be a comma-separated list; take the first.
                let ip_str = s.split(',').next().unwrap_or("").trim();
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // Fall back to the connection's remote address stored by the harness.
    request
        .extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip())
}
