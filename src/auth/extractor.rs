use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;

use crate::auth::jwt;
use crate::error::ApiError;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub merchant_id: Option<uuid::Uuid>,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, Json<ApiError>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(ApiError { error: "missing Authorization header".into() })))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(ApiError { error: "invalid Authorization header".into() })))?;
        let claims = jwt::verify(&state.jwt_secret, token)
            .map_err(|_| (StatusCode::UNAUTHORIZED, Json(ApiError { error: "invalid or expired token".into() })))?;
        Ok(AuthUser {
            user_id: claims.sub,
            merchant_id: claims.merchant_id,
        })
    }
}
