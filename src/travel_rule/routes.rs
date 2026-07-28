use crate::middleware::rbac::extract_identity;
use crate::travel_rule::handlers::*;
use axum::{
    middleware,
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

pub fn travel_rule_router(state: Arc<TravelRuleState>) -> Router {
    // ── Admin routes — require X-User-Id / X-User-Role headers ───────────────
    let admin_routes = Router::new()
        .route(
            "/api/admin/compliance/travel-rule/vasps",
            post(register_vasp).get(list_vasps),
        )
        .route(
            "/api/admin/compliance/travel-rule/vasps/:vasp_id",
            get(get_vasp).patch(update_vasp),
        )
        .route(
            "/api/admin/compliance/travel-rule/thresholds",
            get(get_thresholds).patch(update_thresholds),
        )
        .route(
            "/api/admin/compliance/travel-rule/unhosted-wallet-policy",
            get(get_unhosted_policy).patch(update_unhosted_policy),
        )
        .route(
            "/api/admin/compliance/travel-rule/messages",
            get(list_messages),
        )
        .route(
            "/api/admin/compliance/travel-rule/messages/:message_id",
            get(get_message),
        )
        .route(
            "/api/admin/compliance/travel-rule/messages/:message_id/retry",
            post(retry_message),
        )
        .route(
            "/api/admin/compliance/travel-rule/metrics",
            get(get_metrics),
        )
        // Inject CallerIdentity extension on all admin routes
        .route_layer(middleware::from_fn(extract_identity))
        .with_state(state.clone());

    // ── Public routes — no auth (called by counterpart VASPs and users) ───────
    let public_routes = Router::new()
        .route(
            "/api/compliance/travel-rule/inbound/:protocol",
            post(receive_inbound),
        )
        .route(
            "/api/compliance/travel-rule/attest",
            post(submit_attestation),
        )
        .with_state(state);

    admin_routes.merge(public_routes)
}
