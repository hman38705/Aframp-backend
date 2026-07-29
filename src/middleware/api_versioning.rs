//! API versioning and deprecation lifecycle middleware (Issue #753).
//!
//! # Responsibilities
//!
//! 1. **Version routing** — routes are nested under `/api/v1/` (current).
//!    Future versions add a new nested router under `/api/v2/`, etc.
//!
//! 2. **Deprecation headers** — for any route registered as deprecated, this
//!    middleware injects the following RFC 8594 / RFC 7231 response headers:
//!
//!    ```text
//!    Deprecation: <RFC 7231 HTTP-date of deprecation>
//!    Sunset:      <RFC 7231 HTTP-date of planned removal>
//!    Link:        <https://docs.aframp.io/api/migration>; rel="deprecation"
//!    ```
//!
//! 3. **Retirement (410 Gone)** — routes that have been retired are rejected
//!    at the middleware level with `410 Gone` and a migration guide link,
//!    before the request reaches any handler.
//!
//! 4. **Maintenance header** — routes in `maintenance` status receive an
//!    `X-API-Version-Status: maintenance` response header.
//!
//! # Usage
//!
//! ```rust,no_run
//! use crate::middleware::api_versioning::{
//!     ApiVersionConfig, ApiVersionStatus, deprecation_headers_middleware,
//! };
//! use axum::{Router, routing::get};
//!
//! // Register deprecated routes with their config:
//! let cfg = ApiVersionConfig::deprecated(
//!     "2025-01-01T00:00:00Z",  // deprecation date (RFC 3339)
//!     "2025-07-01T00:00:00Z",  // sunset date
//!     "https://docs.aframp.io/api/migration",
//! );
//!
//! let router = Router::new()
//!     .route("/old-endpoint", get(handler))
//!     .layer(axum::middleware::from_fn_with_state(cfg, deprecation_headers_middleware));
//! ```

use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Version lifecycle states
// ---------------------------------------------------------------------------

/// The support lifecycle state of an API version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiVersionStatus {
    /// Actively developed and fully supported.
    Current,
    /// Security patches only — no new features.
    Maintenance,
    /// End-of-life announced; sunset date set; consumers notified.
    Deprecated,
    /// Completely decommissioned — all traffic rejected with 410.
    Retired,
}

// ---------------------------------------------------------------------------
// Per-version configuration
// ---------------------------------------------------------------------------

/// Configuration attached to a set of routes to describe their lifecycle state.
///
/// Clone is cheap — all string fields are `Arc<str>` internally via `String`.
#[derive(Debug, Clone)]
pub struct ApiVersionConfig {
    /// Current lifecycle status of this version's routes.
    pub status: ApiVersionStatus,
    /// RFC 7231 HTTP-date (or ISO 8601) when this version was deprecated.
    /// Required when `status` is `Deprecated` or `Retired`.
    pub deprecation_date: Option<String>,
    /// RFC 7231 HTTP-date (or ISO 8601) when this version will be / was retired.
    /// Required when `status` is `Deprecated` or `Retired`.
    pub sunset_date: Option<String>,
    /// URL pointing to the migration guide for this version.
    pub migration_url: String,
    /// Human-readable version label, e.g. `"v1"`.
    pub version_label: String,
}

impl ApiVersionConfig {
    /// Create a config for a **current** (actively-supported) API version.
    pub fn current(version_label: impl Into<String>) -> Self {
        Self {
            status: ApiVersionStatus::Current,
            deprecation_date: None,
            sunset_date: None,
            migration_url: "https://docs.aframp.io/api/migration".to_string(),
            version_label: version_label.into(),
        }
    }

    /// Create a config for a **maintenance** API version (security patches only).
    pub fn maintenance(
        version_label: impl Into<String>,
        migration_url: impl Into<String>,
    ) -> Self {
        Self {
            status: ApiVersionStatus::Maintenance,
            deprecation_date: None,
            sunset_date: None,
            migration_url: migration_url.into(),
            version_label: version_label.into(),
        }
    }

    /// Create a config for a **deprecated** API version.
    ///
    /// - `deprecation_date`: when deprecation was announced (RFC 3339 / HTTP-date)
    /// - `sunset_date`: planned retirement date (RFC 3339 / HTTP-date)
    /// - `migration_url`: link to migration guide
    pub fn deprecated(
        version_label: impl Into<String>,
        deprecation_date: impl Into<String>,
        sunset_date: impl Into<String>,
        migration_url: impl Into<String>,
    ) -> Self {
        Self {
            status: ApiVersionStatus::Deprecated,
            deprecation_date: Some(deprecation_date.into()),
            sunset_date: Some(sunset_date.into()),
            migration_url: migration_url.into(),
            version_label: version_label.into(),
        }
    }

    /// Create a config for a **retired** API version.
    ///
    /// All requests to routes carrying this config will be rejected with
    /// `410 Gone` before reaching any handler.
    pub fn retired(
        version_label: impl Into<String>,
        sunset_date: impl Into<String>,
        migration_url: impl Into<String>,
    ) -> Self {
        Self {
            status: ApiVersionStatus::Retired,
            deprecation_date: None,
            sunset_date: Some(sunset_date.into()),
            migration_url: migration_url.into(),
            version_label: version_label.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Axum middleware
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct RetiredResponse {
    error: String,
    message: String,
    migration_url: String,
    version: String,
}

/// Axum middleware that enforces API version lifecycle on a set of routes.
///
/// Attach this with `axum::middleware::from_fn_with_state(config, deprecation_headers_middleware)`
/// to any sub-router that should carry version lifecycle semantics.
///
/// Behaviour by status:
///
/// | Status        | Action                                                       |
/// |---------------|--------------------------------------------------------------|
/// | `Current`     | Pass through; no extra headers.                              |
/// | `Maintenance` | Pass through; adds `X-API-Version-Status: maintenance`.      |
/// | `Deprecated`  | Pass through; adds `Deprecation`, `Sunset`, `Link` headers.  |
/// | `Retired`     | Short-circuits with `410 Gone`; never reaches the handler.   |
pub async fn deprecation_headers_middleware(
    State(config): State<ApiVersionConfig>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    // Retired routes: reject immediately with 410.
    if config.status == ApiVersionStatus::Retired {
        let body = RetiredResponse {
            error: "API_VERSION_RETIRED".to_string(),
            message: format!(
                "API version {} has been retired and is no longer available. \
                 Please migrate to the current version.",
                config.version_label
            ),
            migration_url: config.migration_url.clone(),
            version: config.version_label.clone(),
        };
        return (StatusCode::GONE, Json(body)).into_response();
    }

    // All other statuses: let the request through, then annotate the response.
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();

    match config.status {
        ApiVersionStatus::Maintenance => {
            if let Ok(val) = "maintenance".parse() {
                headers.insert("x-api-version-status", val);
            }
        }
        ApiVersionStatus::Deprecated => {
            // Deprecation header — the date this version was announced deprecated.
            if let Some(ref dep_date) = config.deprecation_date {
                if let Ok(val) = dep_date.parse() {
                    headers.insert("deprecation", val);
                }
            }
            // Sunset header — the planned retirement date (RFC 8594).
            if let Some(ref sunset) = config.sunset_date {
                if let Ok(val) = sunset.parse() {
                    headers.insert("sunset", val);
                }
            }
            // Link header pointing to the migration guide.
            let link_value = format!(
                "<{}>; rel=\"deprecation\", <{}>; rel=\"successor-version\"",
                config.migration_url,
                config.migration_url,
            );
            if let Ok(val) = link_value.parse() {
                headers.insert("link", val);
            }
            // Also surface the version status for easy inspection.
            if let Ok(val) = "deprecated".parse() {
                headers.insert("x-api-version-status", val);
            }
        }
        // Current: no extra headers needed.
        ApiVersionStatus::Current | ApiVersionStatus::Retired => {}
    }

    resp
}

// ---------------------------------------------------------------------------
// Version router helpers
// ---------------------------------------------------------------------------

/// Build an Axum sub-router for `/api/v1/` annotated as the **current** version.
///
/// All routes under this router will receive no extra version headers.
/// Pass the `routes` router containing all your v1 handlers.
pub fn v1_router(routes: axum::Router) -> axum::Router {
    let cfg = ApiVersionConfig::current("v1");
    routes.layer(axum::middleware::from_fn_with_state(
        cfg,
        deprecation_headers_middleware,
    ))
}

/// Build an Axum sub-router for a **deprecated** version.
///
/// All routes under this router will receive `Deprecation`, `Sunset`, and
/// `Link` response headers.
///
/// # Parameters
/// - `version_label`: e.g. `"v0"`
/// - `deprecation_date`: RFC 3339 date string (e.g. `"2025-01-01T00:00:00Z"`)
/// - `sunset_date`: RFC 3339 date string (e.g. `"2025-07-01T00:00:00Z"`)
/// - `migration_url`: URL to migration docs
/// - `routes`: the router containing all the deprecated handlers
pub fn deprecated_version_router(
    version_label: impl Into<String>,
    deprecation_date: impl Into<String>,
    sunset_date: impl Into<String>,
    migration_url: impl Into<String>,
    routes: axum::Router,
) -> axum::Router {
    let cfg = ApiVersionConfig::deprecated(version_label, deprecation_date, sunset_date, migration_url);
    routes.layer(axum::middleware::from_fn_with_state(
        cfg,
        deprecation_headers_middleware,
    ))
}

/// Build an Axum sub-router for a **retired** version.
///
/// Every request to any route under this router is rejected with `410 Gone`
/// before the handler is called.
pub fn retired_version_router(
    version_label: impl Into<String>,
    sunset_date: impl Into<String>,
    migration_url: impl Into<String>,
    routes: axum::Router,
) -> axum::Router {
    let cfg = ApiVersionConfig::retired(version_label, sunset_date, migration_url);
    routes.layer(axum::middleware::from_fn_with_state(
        cfg,
        deprecation_headers_middleware,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use axum_test::TestServer;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn make_server(config: ApiVersionConfig) -> TestServer {
        let app = Router::new()
            .route("/test", get(ok_handler))
            .layer(axum::middleware::from_fn_with_state(
                config,
                deprecation_headers_middleware,
            ));
        TestServer::new(app).unwrap()
    }

    #[tokio::test]
    async fn current_version_has_no_extra_headers() {
        let server = make_server(ApiVersionConfig::current("v1"));
        let resp = server.get("/test").await;
        resp.assert_status_ok();
        assert!(resp.headers().get("deprecation").is_none());
        assert!(resp.headers().get("sunset").is_none());
        assert!(resp.headers().get("x-api-version-status").is_none());
    }

    #[tokio::test]
    async fn deprecated_version_adds_deprecation_headers() {
        let server = make_server(ApiVersionConfig::deprecated(
            "v0",
            "2025-01-01T00:00:00Z",
            "2025-07-01T00:00:00Z",
            "https://docs.aframp.io/api/migration",
        ));
        let resp = server.get("/test").await;
        resp.assert_status_ok();
        assert!(resp.headers().get("deprecation").is_some());
        assert!(resp.headers().get("sunset").is_some());
        assert!(resp.headers().get("link").is_some());
        assert_eq!(
            resp.headers().get("x-api-version-status").unwrap(),
            "deprecated"
        );
    }

    #[tokio::test]
    async fn retired_version_returns_410() {
        let server = make_server(ApiVersionConfig::retired(
            "v0",
            "2025-01-01T00:00:00Z",
            "https://docs.aframp.io/api/migration",
        ));
        let resp = server.get("/test").await;
        resp.assert_status(StatusCode::GONE);
    }

    #[tokio::test]
    async fn maintenance_version_adds_status_header() {
        let server = make_server(ApiVersionConfig::maintenance(
            "v1",
            "https://docs.aframp.io/api/migration",
        ));
        let resp = server.get("/test").await;
        resp.assert_status_ok();
        assert_eq!(
            resp.headers().get("x-api-version-status").unwrap(),
            "maintenance"
        );
    }
}
