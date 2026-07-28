//! Security headers layer (Issue #724)
//!
//! Injects HTTP security headers on every response so the application-layer
//! guarantees are present regardless of whether nginx/ingress is in front.
//!
//! # Headers applied
//! | Header                        | Value (production)                            |
//! |-------------------------------|-----------------------------------------------|
//! | `Strict-Transport-Security`   | `max-age=31536000; includeSubDomains; preload` |
//! | `X-Content-Type-Options`      | `nosniff`                                     |
//! | `X-Frame-Options`             | `DENY`                                        |
//! | `X-XSS-Protection`            | `1; mode=block`                               |
//! | `Referrer-Policy`             | `strict-origin-when-cross-origin`             |
//! | `Permissions-Policy`          | restrictive feature set                       |
//! | `Content-Security-Policy`     | strict in prod/staging, relaxed in dev        |
//!
//! # Per-environment behaviour
//! - **production / staging**: HSTS applied, CSP disallows `unsafe-eval`.
//! - **development / test**: HSTS omitted, CSP allows `unsafe-eval`.
//!
//! # Integration
//! ```rust,ignore
//! let cfg = SecurityHeadersConfig::from_env();
//! let app = Router::new()
//!     ./* ... */
//!     .layer(axum::middleware::from_fn_with_state(
//!         cfg,
//!         security_headers_middleware,
//!     ));
//! ```

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, Response},
    middleware::Next,
};
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// Deployment environment
// ---------------------------------------------------------------------------

/// Deployment environment tier used to tune security header strictness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEnvironment {
    Development,
    Staging,
    Production,
    Test,
}

impl AppEnvironment {
    /// Derive from `APP_ENV` / `ENVIRONMENT` env vars. Defaults to `Development`.
    pub fn from_env() -> Self {
        let raw = std::env::var("APP_ENV")
            .or_else(|_| std::env::var("ENVIRONMENT"))
            .unwrap_or_else(|_| "development".to_string());
        match raw.to_lowercase().trim() {
            "production" | "prod" => Self::Production,
            "staging" | "stage" => Self::Staging,
            "test" => Self::Test,
            _ => Self::Development,
        }
    }

    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    /// `true` for production and staging — environments where HSTS and strict
    /// CSP should be enforced.
    pub fn is_production_like(&self) -> bool {
        matches!(self, Self::Production | Self::Staging)
    }

    pub fn is_development(&self) -> bool {
        matches!(self, Self::Development | Self::Test)
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the security headers middleware.
///
/// Loaded from environment variables; also constructible directly for tests.
#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    /// Deployment environment — controls HSTS and CSP strictness.
    pub environment: AppEnvironment,
    /// Whether to emit `Strict-Transport-Security`. Auto-set for prod/staging
    /// when TLS is detected; overridable via `SECURITY_ENABLE_HSTS`.
    pub enable_hsts: bool,
    /// HSTS `max-age` in seconds. Default: 31 536 000 (1 year).
    pub hsts_max_age_secs: u64,
    /// Fully custom `Content-Security-Policy` value. When set it replaces the
    /// built-in per-environment policy entirely.
    pub custom_csp: Option<String>,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        let environment = AppEnvironment::from_env();
        let enable_hsts = environment.is_production_like() && is_https_environment();
        Self {
            environment,
            enable_hsts,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        }
    }
}

impl SecurityHeadersConfig {
    /// Load from environment variables.
    ///
    /// | Variable                | Default                  | Description                             |
    /// |-------------------------|--------------------------|-----------------------------------------|
    /// | `APP_ENV`               | `development`            | Deployment environment                  |
    /// | `SECURITY_ENABLE_HSTS`  | auto (prod + HTTPS)      | Force-enable or disable HSTS            |
    /// | `SECURITY_HSTS_MAX_AGE` | `31536000`               | HSTS max-age (seconds)                  |
    /// | `SECURITY_CUSTOM_CSP`   | *(not set)*              | Override Content-Security-Policy value  |
    pub fn from_env() -> Self {
        let environment = AppEnvironment::from_env();
        let default_hsts = environment.is_production_like() && is_https_environment();

        let enable_hsts = std::env::var("SECURITY_ENABLE_HSTS")
            .ok()
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(default_hsts);

        let hsts_max_age_secs = std::env::var("SECURITY_HSTS_MAX_AGE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(31_536_000u64);

        let custom_csp = std::env::var("SECURITY_CUSTOM_CSP").ok();

        Self {
            environment,
            enable_hsts,
            hsts_max_age_secs,
            custom_csp,
        }
    }

    /// Build the `Content-Security-Policy` header value for this config.
    ///
    /// Returns the `custom_csp` override if set; otherwise builds a sensible
    /// default tuned to the deployment environment.
    pub fn build_csp(&self) -> String {
        if let Some(ref custom) = self.custom_csp {
            return custom.clone();
        }

        let mut directives = vec![
            "default-src 'self'",
            "style-src 'self' 'unsafe-inline'",
            "img-src 'self' data: https:",
            "font-src 'self'",
            "connect-src 'self'",
            "media-src 'none'",
            "object-src 'none'",
            "child-src 'none'",
            "frame-src 'none'",
            "worker-src 'none'",
            "frame-ancestors 'none'",
            "form-action 'self'",
            "base-uri 'self'",
            "manifest-src 'self'",
        ];

        if self.environment.is_development() {
            // Allow eval only in development (e.g. hot-reload tooling)
            directives.push("script-src 'self' 'unsafe-eval'");
        } else {
            directives.push("script-src 'self'");
        }

        directives.join("; ")
    }

    /// Build the `Strict-Transport-Security` header value for this config,
    /// returning `None` if HSTS is disabled.
    pub fn build_hsts(&self) -> Option<String> {
        if !self.enable_hsts {
            return None;
        }
        Some(format!(
            "max-age={}; includeSubDomains; preload",
            self.hsts_max_age_secs
        ))
    }
}

// ---------------------------------------------------------------------------
// HTTPS detection helper
// ---------------------------------------------------------------------------

fn is_https_environment() -> bool {
    std::env::var("HTTPS").unwrap_or_default().to_lowercase() == "true"
        || std::env::var("TLS_ENABLED")
            .unwrap_or_default()
            .to_lowercase()
            == "true"
        || std::env::var("SSL_ENABLED")
            .unwrap_or_default()
            .to_lowercase()
            == "true"
        || std::env::var("SERVER_URL")
            .unwrap_or_default()
            .starts_with("https://")
}

// ---------------------------------------------------------------------------
// Middleware (stateful — config injected via State)
// ---------------------------------------------------------------------------

/// Axum middleware that writes all security headers onto every response.
///
/// Attach using `axum::middleware::from_fn_with_state(config, security_headers_middleware)`.
pub async fn security_headers_middleware(
    State(config): State<SecurityHeadersConfig>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let mut response = next.run(request).await;
    apply_security_headers(&mut response, &config);
    response
}

/// Stateless variant that reads `SecurityHeadersConfig::from_env()` per
/// request.  Useful when you cannot pass state through a layer.
pub async fn security_headers_middleware_stateless(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let config = SecurityHeadersConfig::from_env();
    let mut response = next.run(request).await;
    apply_security_headers(&mut response, &config);
    response
}

// ---------------------------------------------------------------------------
// Core injection function (public for testing)
// ---------------------------------------------------------------------------

/// Write all security headers onto `response`.
///
/// This function is `pub` so tests can call it directly without spinning up a
/// full Axum router.
pub fn apply_security_headers(response: &mut Response<Body>, config: &SecurityHeadersConfig) {
    let headers = response.headers_mut();

    // ── Always-on headers ────────────────────────────────────────────────────

    // Prevent clickjacking
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));

    // Prevent MIME-type sniffing attacks
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );

    // Legacy XSS filter (belt-and-suspenders for older browsers)
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );

    // Restrict Referer header leakage
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Disable access to sensitive browser features
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static(
            "geolocation=(), microphone=(), camera=(), payment=(), usb=()",
        ),
    );

    // Obscure server technology
    headers.remove("X-Powered-By");
    headers.insert("Server", HeaderValue::from_static("Aframp API"));

    // ── Content-Security-Policy (per-environment) ────────────────────────────
    let csp = config.build_csp();
    if let Ok(csp_value) = HeaderValue::from_str(&csp) {
        headers.insert("Content-Security-Policy", csp_value);
    }

    // ── Strict-Transport-Security (production/staging + HTTPS only) ──────────
    if let Some(hsts) = config.build_hsts() {
        if let Ok(hsts_value) = HeaderValue::from_str(&hsts) {
            headers.insert("Strict-Transport-Security", hsts_value);
            info!(
                environment = ?config.environment,
                max_age = config.hsts_max_age_secs,
                "HSTS header applied"
            );
        }
    }

    // ── Cache-Control: all API responses private by default ──────────────────
    headers.insert(
        "Cache-Control",
        HeaderValue::from_static("no-store, private"),
    );

    debug!(environment = ?config.environment, "Security headers applied to response");
}

// ---------------------------------------------------------------------------
// Legacy Security configuration struct (kept for backwards compatibility)
// ---------------------------------------------------------------------------

/// Legacy configuration structure — use [`SecurityHeadersConfig`] for new code.
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub enable_hsts: bool,
    pub hsts_max_age: u32,
    pub enable_csp: bool,
    pub custom_csp: Option<String>,
    pub hide_server_header: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 31_536_000,
            enable_csp: true,
            custom_csp: None,
            hide_server_header: false,
        }
    }
}

impl SecurityConfig {
    pub fn from_env() -> Self {
        Self {
            enable_hsts: std::env::var("SECURITY_ENABLE_HSTS")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                == "true",
            hsts_max_age: std::env::var("SECURITY_HSTS_MAX_AGE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(31_536_000),
            enable_csp: std::env::var("SECURITY_ENABLE_CSP")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                == "true",
            custom_csp: std::env::var("SECURITY_CUSTOM_CSP").ok(),
            hide_server_header: std::env::var("SECURITY_HIDE_SERVER")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn make_response() -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap()
    }

    // ── AppEnvironment ───────────────────────────────────────────────────────

    #[test]
    fn app_env_defaults_to_development() {
        std::env::remove_var("APP_ENV");
        std::env::remove_var("ENVIRONMENT");
        assert_eq!(AppEnvironment::from_env(), AppEnvironment::Development);
    }

    #[test]
    fn app_env_parses_production() {
        std::env::set_var("APP_ENV", "production");
        assert!(AppEnvironment::from_env().is_production());
        std::env::remove_var("APP_ENV");
    }

    #[test]
    fn app_env_parses_staging_as_production_like() {
        std::env::set_var("APP_ENV", "staging");
        assert!(AppEnvironment::from_env().is_production_like());
        std::env::remove_var("APP_ENV");
    }

    #[test]
    fn development_is_not_production() {
        std::env::set_var("APP_ENV", "development");
        let env = AppEnvironment::from_env();
        assert!(!env.is_production());
        assert!(env.is_development());
        std::env::remove_var("APP_ENV");
    }

    // ── SecurityHeadersConfig::build_hsts ────────────────────────────────────

    #[test]
    fn hsts_disabled_in_development() {
        let cfg = SecurityHeadersConfig {
            environment: AppEnvironment::Development,
            enable_hsts: false,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        assert!(cfg.build_hsts().is_none());
    }

    #[test]
    fn hsts_enabled_in_production() {
        let cfg = SecurityHeadersConfig {
            environment: AppEnvironment::Production,
            enable_hsts: true,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        let hsts = cfg.build_hsts().unwrap();
        assert!(hsts.contains("max-age=31536000"));
        assert!(hsts.contains("includeSubDomains"));
        assert!(hsts.contains("preload"));
    }

    // ── SecurityHeadersConfig::build_csp ─────────────────────────────────────

    #[test]
    fn csp_production_has_no_unsafe_eval() {
        let cfg = SecurityHeadersConfig {
            environment: AppEnvironment::Production,
            enable_hsts: true,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        let csp = cfg.build_csp();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(!csp.contains("unsafe-eval"));
    }

    #[test]
    fn csp_development_allows_unsafe_eval() {
        let cfg = SecurityHeadersConfig {
            environment: AppEnvironment::Development,
            enable_hsts: false,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        let csp = cfg.build_csp();
        assert!(csp.contains("unsafe-eval"));
    }

    #[test]
    fn custom_csp_overrides_defaults() {
        let cfg = SecurityHeadersConfig {
            environment: AppEnvironment::Production,
            enable_hsts: true,
            hsts_max_age_secs: 31_536_000,
            custom_csp: Some("default-src 'none'".to_string()),
        };
        assert_eq!(cfg.build_csp(), "default-src 'none'");
    }

    // ── apply_security_headers ───────────────────────────────────────────────

    #[test]
    fn headers_present_on_every_response() {
        let mut resp = make_response();
        let cfg = SecurityHeadersConfig {
            environment: AppEnvironment::Production,
            enable_hsts: false,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        apply_security_headers(&mut resp, &cfg);

        assert_eq!(resp.headers().get("X-Frame-Options").unwrap(), "DENY");
        assert_eq!(
            resp.headers().get("X-Content-Type-Options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            resp.headers().get("X-XSS-Protection").unwrap(),
            "1; mode=block"
        );
        assert_eq!(
            resp.headers().get("Referrer-Policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert!(resp.headers().get("Content-Security-Policy").is_some());
        assert!(resp.headers().get("Permissions-Policy").is_some());
    }

    #[test]
    fn hsts_only_present_when_enabled() {
        // Disabled
        let mut resp = make_response();
        let no_hsts = SecurityHeadersConfig {
            environment: AppEnvironment::Development,
            enable_hsts: false,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        apply_security_headers(&mut resp, &no_hsts);
        assert!(resp.headers().get("Strict-Transport-Security").is_none());

        // Enabled
        let mut resp2 = make_response();
        let with_hsts = SecurityHeadersConfig {
            environment: AppEnvironment::Production,
            enable_hsts: true,
            hsts_max_age_secs: 31_536_000,
            custom_csp: None,
        };
        apply_security_headers(&mut resp2, &with_hsts);
        let sts = resp2
            .headers()
            .get("Strict-Transport-Security")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(sts.starts_with("max-age=31536000"));
    }

    #[test]
    fn x_powered_by_is_removed() {
        let mut resp = Response::builder()
            .status(StatusCode::OK)
            .header("X-Powered-By", "Rocket")
            .body(Body::empty())
            .unwrap();
        let cfg = SecurityHeadersConfig::default();
        apply_security_headers(&mut resp, &cfg);
        assert!(resp.headers().get("X-Powered-By").is_none());
    }

    // ── SecurityHeadersConfig::from_env ──────────────────────────────────────

    #[test]
    fn from_env_reads_custom_csp() {
        std::env::set_var("SECURITY_CUSTOM_CSP", "default-src 'none'");
        let cfg = SecurityHeadersConfig::from_env();
        assert_eq!(cfg.build_csp(), "default-src 'none'");
        std::env::remove_var("SECURITY_CUSTOM_CSP");
    }

    #[test]
    fn from_env_reads_hsts_max_age() {
        std::env::set_var("SECURITY_HSTS_MAX_AGE", "86400");
        std::env::set_var("SECURITY_ENABLE_HSTS", "true");
        let cfg = SecurityHeadersConfig::from_env();
        assert_eq!(cfg.hsts_max_age_secs, 86400);
        let hsts = cfg.build_hsts().unwrap();
        assert!(hsts.contains("max-age=86400"));
        std::env::remove_var("SECURITY_HSTS_MAX_AGE");
        std::env::remove_var("SECURITY_ENABLE_HSTS");
    }
}
