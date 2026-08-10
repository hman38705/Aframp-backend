mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::{send, state};

async fn app() -> Option<axum::Router> {
    state().await.map(aframp::router)
}

#[tokio::test]
async fn signup_and_login_success() {
    let Some(app) = app().await else {
        return;
    };
    let email = format!("alice+{}@example.com", uuid::Uuid::new_v4().simple());

    let (status, json) = send(
        app.clone(),
        "POST",
        "/signup",
        None,
        Some(json!({ "email": email, "password": "password123", "name": "Alice" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "signup failed: {json}");
    assert!(json["token"].as_str().unwrap().len() > 10);
    assert!(json["merchant_id"].as_str().is_some());

    let (status, json) = send(
        app.clone(),
        "POST",
        "/login",
        None,
        Some(json!({ "email": email, "password": "password123" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login failed: {json}");
    assert!(json["token"].as_str().unwrap().len() > 10);
}

#[tokio::test]
async fn signup_duplicate_email_conflicts() {
    let Some(app) = app().await else {
        return;
    };
    let email = format!("dup+{}@example.com", uuid::Uuid::new_v4().simple());

    send(
        app.clone(),
        "POST",
        "/signup",
        None,
        Some(json!({ "email": email, "password": "password123", "name": "Dup" })),
    )
    .await;

    let (status, json) = send(
        app.clone(),
        "POST",
        "/signup",
        None,
        Some(json!({ "email": email, "password": "password123", "name": "Dup" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "expected conflict: {json}");
    assert_eq!(json["error"], "email already registered");
}

#[tokio::test]
async fn signup_weak_password_rejected() {
    let Some(app) = app().await else {
        return;
    };
    let (status, _) = send(
        app.clone(),
        "POST",
        "/signup",
        None,
        Some(json!({ "email": "weak@example.com", "password": "short", "name": "Weak" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn login_wrong_password_unauthorized() {
    let Some(app) = app().await else {
        return;
    };
    let email = format!("wrongpw+{}@example.com", uuid::Uuid::new_v4().simple());
    send(
        app.clone(),
        "POST",
        "/signup",
        None,
        Some(json!({ "email": email, "password": "password123", "name": "Bob" })),
    )
    .await;

    let (status, _) = send(
        app.clone(),
        "POST",
        "/login",
        None,
        Some(json!({ "email": email, "password": "not-the-password" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
