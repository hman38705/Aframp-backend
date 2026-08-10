use axum::extract::State;
use axum::Json;

use crate::auth::jwt;
use crate::error::{bad_request, internal, ApiResult};
use crate::models::{AuthResponse, LoginRequest, SignupRequest};
use crate::services::users::{self, UserError};
use crate::AppState;

pub async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> ApiResult<Json<AuthResponse>> {
    if req.email.is_empty() || req.password.len() < 8 || req.name.is_empty() {
        return Err(bad_request("email, a password of at least 8 characters, and name are required"));
    }
    let (user, merchant) = users::signup(&state.db, &req.email, &req.password, &req.name)
        .await
        .map_err(map_user_error)?;
    let token = jwt::sign(&state.jwt_secret, user.id, Some(merchant.id))
        .map_err(internal)?;
    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        merchant_id: Some(merchant.id),
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let (user, merchant) = users::login(&state.db, &req.email, &req.password)
        .await
        .map_err(map_user_error)?;
    let token = jwt::sign(&state.jwt_secret, user.id, merchant.as_ref().map(|m| m.id))
        .map_err(internal)?;
    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        merchant_id: merchant.map(|m| m.id),
    }))
}

fn map_user_error(err: UserError) -> (axum::http::StatusCode, Json<crate::error::ApiError>) {
    match err {
        UserError::EmailTaken => crate::error::conflict("email already registered"),
        UserError::InvalidCredentials => crate::error::unauthorized("invalid email or password"),
        _ => internal(err),
    }
}
