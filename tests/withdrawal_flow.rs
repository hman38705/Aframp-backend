mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::{ensure_merchant, send, state};

#[tokio::test]
async fn withdrawal_insufficient_balance_rejected() {
    let Some(state) = state().await else {
        return;
    };
    let app = aframp::router(state.clone());
    let (token, _) = ensure_merchant(&app, "insufficient").await;

    let (status, json) = send(
        app.clone(),
        "POST",
        "/withdraw",
        Some(&token),
        Some(json!({
            "amount_stroops": 1_000_000,
            "asset": "cNGN",
            "bank_code": "058",
            "account_number": "0123456789"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "expected rejection: {json}");
    assert_eq!(json["error"], "insufficient available balance");
}

#[tokio::test]
async fn withdrawal_validates_bank_details() {
    let Some(state) = state().await else {
        return;
    };
    let app = aframp::router(state.clone());
    let (token, _) = ensure_merchant(&app, "validation").await;

    let (status, _) = send(
        app.clone(),
        "POST",
        "/withdraw",
        Some(&token),
        Some(json!({
            "amount_stroops": 1_000_000,
            "bank_code": "",
            "account_number": "123"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn withdrawal_success_decrements_balance() {
    let Some(state) = state().await else {
        return;
    };
    let app = aframp::router(state.clone());
    let (token, merchant_id) = ensure_merchant(&app, "withdraw_ok").await;

    sqlx::query(
        "INSERT INTO balances (merchant_id, asset, available, pending)
         VALUES ($1::uuid, 'cNGN', 5_000_000, 0)
         ON CONFLICT (merchant_id, asset) DO UPDATE SET available = 5_000_000, pending = 0",
    )
    .bind(&merchant_id)
    .execute(&state.db)
    .await
    .unwrap();

    let (status, json) = send(
        app.clone(),
        "POST",
        "/withdraw",
        Some(&token),
        Some(json!({
            "amount_stroops": 2_000_000,
            "asset": "cNGN",
            "bank_code": "058",
            "account_number": "0123456789"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "withdraw failed: {json}");
    assert_eq!(json["status"], "pending");
    assert_eq!(json["amount_stroops"], 2_000_000);

    let balance = sqlx::query_scalar::<_, i64>(
        "SELECT available FROM balances WHERE merchant_id = $1::uuid AND asset = 'cNGN'",
    )
    .bind(&merchant_id)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(balance, 3_000_000, "available balance should be debited");

    let (status, json) = send(app.clone(), "GET", "/withdrawals", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn withdrawal_full_balance_then_insufficient() {
    let Some(state) = state().await else {
        return;
    };
    let app = aframp::router(state.clone());
    let (token, merchant_id) = ensure_merchant(&app, "drain").await;

    sqlx::query(
        "INSERT INTO balances (merchant_id, asset, available, pending)
         VALUES ($1::uuid, 'cNGN', 1_000_000, 0)",
    )
    .bind(&merchant_id)
    .execute(&state.db)
    .await
    .unwrap();

    let (status, _) = send(
        app.clone(),
        "POST",
        "/withdraw",
        Some(&token),
        Some(json!({
            "amount_stroops": 1_000_000,
            "bank_code": "058",
            "account_number": "0123456789"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        app.clone(),
        "POST",
        "/withdraw",
        Some(&token),
        Some(json!({
            "amount_stroops": 1,
            "bank_code": "058",
            "account_number": "0123456789"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "second withdrawal should fail");
}
