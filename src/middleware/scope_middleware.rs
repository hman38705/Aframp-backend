//! OAuth 2.0 Scope Enforcement Middleware
//!
//! Validates that requests have required scopes before allowing access.
//! Supports single scope, multi-scope (ALL), and scope hierarchy.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::auth::oauth_token_validator::OAuthTokenClaims;
use crate::auth::scope_hierarchy::ScopeHierarchy;

// ── Scope Requirement ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ScopeRequirement {
    /// Single scope required
    Single(String),
    /// All scopes required
    All(Vec<String>),
    /// Any scope required
    Any(Vec<String>),
}

impl ScopeRequirement {
    pub fn single(scope: &str) -> Self {
        ScopeRequirement::Single(scope.to_string())
    }

    pub fn all(scopes: Vec<&str>) -> Self {
        ScopeRequirement::All(scopes.iter().map(|s| s.to_string()).collect())
    }

    pub fn any(scopes: Vec<&str>) -> Self {
        ScopeRequirement::Any(scopes.iter().map(|s| s.to_string()).collect())
    }
}

// ── Scope Enforcement Error ──────────────────────────────────────────────────

#[derive(Debug)]
pub struct ScopeEnforcementError {
    pub required_scope: String,
    pub granted_scopes: String,
}

impl ScopeEnforcementError {
    pub fn response(&self) -> Response {
        (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "insufficient_scope",
                "error_description": "The request requires scopes that were not granted",
                "required_scope": self.required_scope,
                "granted_scopes": self.granted_scopes,
            })),
        )
            .into_response()
    }
}

// ── Scope Enforcement Middleware ─────────────────────────────────────────────

/// Enforce single scope requirement
pub async fn enforce_single_scope(scope: String, mut req: Request, next: Next) -> Response {
    // Extract claims from extensions (set by token validator)
    let claims = match req.extensions().get::<OAuthTokenClaims>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "missing_token",
                    "error_description": "No valid token found"
                })),
            )
                .into_response();
        }
    };

    // Parse granted scopes
    let granted_scopes: Vec<&str> = claims.scope.split_whitespace().collect();

    // Check scope requirement using hierarchy
    let hierarchy = ScopeHierarchy::new();

    if !hierarchy.satisfies(&granted_scopes, &scope) {
        let error = ScopeEnforcementError {
            required_scope: scope,
            granted_scopes: claims.scope.clone(),
        };

        // Log scope denial
        tracing::warn!(
            jti = %claims.jti,
            consumer_id = %claims.sub,
            client_id = %claims.client_id,
            required_scope = %error.required_scope,
            granted_scopes = %error.granted_scopes,
            "scope enforcement denied"
        );

        return error.response();
    }

    // Log scope approval
    tracing::debug!(
        jti = %claims.jti,
        consumer_id = %claims.sub,
        client_id = %claims.client_id,
        scopes = %claims.scope,
        "scope enforcement approved"
    );

    next.run(req).await
}

/// Enforce multiple scopes (ALL must be present)
pub async fn enforce_all_scopes(scopes: Vec<String>, mut req: Request, next: Next) -> Response {
    let claims = match req.extensions().get::<OAuthTokenClaims>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "missing_token",
                    "error_description": "No valid token found"
                })),
            )
                .into_response();
        }
    };

    let granted_scopes: Vec<&str> = claims.scope.split_whitespace().collect();
    let hierarchy = ScopeHierarchy::new();

    let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();

    if !hierarchy.satisfies_all(&granted_scopes, &scope_refs) {
        let error = ScopeEnforcementError {
            required_scope: scopes.join(" AND "),
            granted_scopes: claims.scope.clone(),
        };

        tracing::warn!(
            jti = %claims.jti,
            consumer_id = %claims.sub,
            client_id = %claims.client_id,
            required_scope = %error.required_scope,
            granted_scopes = %error.granted_scopes,
            "scope enforcement denied (all)"
        );

        return error.response();
    }

    next.run(req).await
}

/// Enforce any scope (at least one must be present)
pub async fn enforce_any_scope(scopes: Vec<String>, mut req: Request, next: Next) -> Response {
    let claims = match req.extensions().get::<OAuthTokenClaims>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "missing_token",
                    "error_description": "No valid token found"
                })),
            )
                .into_response();
        }
    };

    let granted_scopes: Vec<&str> = claims.scope.split_whitespace().collect();
    let hierarchy = ScopeHierarchy::new();

    let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();

    if !hierarchy.satisfies_any(&granted_scopes, &scope_refs) {
        let error = ScopeEnforcementError {
            required_scope: scopes.join(" OR "),
            granted_scopes: claims.scope.clone(),
        };

        tracing::warn!(
            jti = %claims.jti,
            consumer_id = %claims.sub,
            client_id = %claims.client_id,
            required_scope = %error.required_scope,
            granted_scopes = %error.granted_scopes,
            "scope enforcement denied (any)"
        );

        return error.response();
    }

    next.run(req).await
}

// ── Startup-time scope registry validation ──────────────────────────────────

/// Validate that every scope referenced by a `ScopeRequirement` used in route
/// definitions is present in the OAuth scope registry
/// (`crate::oauth::types::SUPPORTED_SCOPES`, mirrored from
/// `migrations/20240324_create_oauth_scopes.sql`). Call this once at startup
/// (e.g. in `main`) with the full list of `ScopeRequirement`s wired into the
/// router so an unregistered scope fails fast instead of silently denying
/// requests at runtime.
pub fn validate_registered_scopes(requirements: &[ScopeRequirement]) -> Result<(), Vec<String>> {
    let mut unknown = Vec::new();
    for req in requirements {
        let scopes: Vec<&String> = match req {
            ScopeRequirement::Single(s) => vec![s],
            ScopeRequirement::All(v) | ScopeRequirement::Any(v) => v.iter().collect(),
        };
        for scope in scopes {
            if !crate::oauth::types::is_supported_scope(scope) {
                unknown.push(scope.clone());
            }
        }
    }
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_requirement_single() {
        let req = ScopeRequirement::single("wallet:read");
        match req {
            ScopeRequirement::Single(s) => assert_eq!(s, "wallet:read"),
            _ => panic!("Expected Single"),
        }
    }

    #[test]
    fn test_scope_requirement_all() {
        let req = ScopeRequirement::all(vec!["wallet:read", "onramp:quote"]);
        match req {
            ScopeRequirement::All(scopes) => assert_eq!(scopes.len(), 2),
            _ => panic!("Expected All"),
        }
    }

    #[test]
    fn test_scope_requirement_any() {
        let req = ScopeRequirement::any(vec!["wallet:read", "onramp:quote"]);
        match req {
            ScopeRequirement::Any(scopes) => assert_eq!(scopes.len(), 2),
            _ => panic!("Expected Any"),
        }
    }

    #[test]
    fn test_scope_enforcement_error() {
        let error = ScopeEnforcementError {
            required_scope: "wallet:read".to_string(),
            granted_scopes: "onramp:quote".to_string(),
        };

        let response = error.response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
