//! Integration tests driving the real `Router` in-process via `tower`'s `oneshot`
//! (no TCP port, no `cargo run`). Each test gets a fresh in-memory SQLite DB.
//!
//! Two groups:
//!   * `prompt_*`  — the verification scenarios from `../PROMPT.md`.
//!   * `sqli_*`    — SQL-injection attempts, proving the parameterized queries in
//!                   `repo.rs` are not exploitable.

use std::net::SocketAddr;

use axum::body::{to_bytes, Body};
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt; // brings `oneshot` into scope

use qr_code_generator::state::AppState;
use qr_code_generator::{build_router, run_migrations};

// ---- test harness ---------------------------------------------------------

/// Build a router backed by a private in-memory DB.
/// `max_connections(1)` keeps the single in-memory database alive for the pool's life.
async fn test_app() -> Router {
    let pool = qr_code_generator::init_pool("sqlite::memory:", 1)
        .await
        .expect("pool");
    run_migrations(&pool).await.expect("migrate");
    build_router(AppState::new(pool))
        // The redirect handler extracts ConnectInfo<SocketAddr>; supply a fake peer.
        .layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4321))))
}

struct Resp {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
    fn location(&self) -> Option<String> {
        self.headers
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_string())
    }
    fn content_type(&self) -> Option<String> {
        self.headers
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_string())
    }
}

async fn send(app: &Router, req: Request<Body>) -> Resp {
    let resp = app.clone().oneshot(req).await.expect("router response");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    Resp { status, headers, body }
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn json_req(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Create a link and return its token.
async fn create(app: &Router, url: &str) -> String {
    let r = send(app, json_req("POST", "/api/qr/create", json!({ "url": url }))).await;
    assert_eq!(r.status, StatusCode::OK, "create should succeed for {url}");
    r.json()["token"].as_str().unwrap().to_string()
}

// ===========================================================================
// PROMPT.md verification scenarios
// ===========================================================================

#[tokio::test]
async fn prompt_create_returns_token_and_urls() {
    let app = test_app().await;
    let r = send(&app, json_req("POST", "/api/qr/create", json!({ "url": "https://example.com" }))).await;

    assert_eq!(r.status, StatusCode::OK);
    let body = r.json();
    let token = body["token"].as_str().unwrap();
    assert_eq!(token.len(), 7);
    assert_eq!(body["short_url"], json!(format!("http://localhost:8000/r/{token}")));
    assert_eq!(body["qr_code_url"], json!(format!("http://localhost:8000/api/qr/{token}/image")));
    assert_eq!(body["original_url"], json!("https://example.com"));
}

#[tokio::test]
async fn prompt_redirect_returns_302() {
    let app = test_app().await;
    let token = create(&app, "https://example.com").await;

    let r = send(&app, get(&format!("/r/{token}"))).await;
    assert_eq!(r.status, StatusCode::FOUND); // 302
    assert_eq!(r.location().as_deref(), Some("https://example.com"));
}

#[tokio::test]
async fn prompt_get_info() {
    let app = test_app().await;
    let token = create(&app, "https://example.com").await;

    let r = send(&app, get(&format!("/api/qr/{token}"))).await;
    assert_eq!(r.status, StatusCode::OK);
    let body = r.json();
    assert_eq!(body["token"], json!(token));
    assert_eq!(body["original_url"], json!("https://example.com"));
    assert_eq!(body["is_deleted"], json!(false));
}

#[tokio::test]
async fn prompt_update_changes_redirect_target() {
    let app = test_app().await;
    let token = create(&app, "https://example.com").await;

    let r = send(&app, json_req("PATCH", &format!("/api/qr/{token}"), json!({ "url": "https://new-url.com" }))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["original_url"], json!("https://new-url.com"));

    // Redirect now points to the new target (raw Location, no curl re-resolution).
    let r = send(&app, get(&format!("/r/{token}"))).await;
    assert_eq!(r.status, StatusCode::FOUND);
    assert_eq!(r.location().as_deref(), Some("https://new-url.com"));
}

#[tokio::test]
async fn prompt_delete_then_redirect_is_410() {
    let app = test_app().await;
    let token = create(&app, "https://example.com").await;

    let r = send(&app, Request::builder().method("DELETE").uri(format!("/api/qr/{token}")).body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::OK);

    // Deleted link -> 410 Gone.
    let r = send(&app, get(&format!("/r/{token}"))).await;
    assert_eq!(r.status, StatusCode::GONE);

    // Management endpoints treat a deleted link as 404.
    let r = send(&app, get(&format!("/api/qr/{token}"))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn prompt_nonexistent_token_is_404() {
    let app = test_app().await;
    let r = send(&app, get("/r/INVALID")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn prompt_qr_image_is_png() {
    let app = test_app().await;
    let token = create(&app, "https://example.com").await;

    let r = send(&app, get(&format!("/api/qr/{token}/image"))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.content_type().as_deref(), Some("image/png"));
    // PNG magic number.
    assert_eq!(&r.body[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
}

#[tokio::test]
async fn prompt_analytics_counts_scans() {
    let app = test_app().await;
    let token = create(&app, "https://example.com").await;

    // Two scans.
    send(&app, get(&format!("/r/{token}"))).await;
    send(&app, get(&format!("/r/{token}"))).await;

    let r = send(&app, get(&format!("/api/qr/{token}/analytics"))).await;
    assert_eq!(r.status, StatusCode::OK);
    let body = r.json();
    assert_eq!(body["token"], json!(token));
    assert_eq!(body["total_scans"], json!(2));
    assert_eq!(body["scans_by_day"].as_array().unwrap().len(), 1);
    assert_eq!(body["scans_by_day"][0]["count"], json!(2));
}

// ---- behaviour beyond the curl script -------------------------------------

#[tokio::test]
async fn expired_link_is_410_future_is_302() {
    let app = test_app().await;

    let expired = send(&app, json_req("POST", "/api/qr/create",
        json!({ "url": "https://expired.com", "expires_at": "2020-01-01T00:00:00Z" }))).await;
    let etoken = expired.json()["token"].as_str().unwrap().to_string();
    let r = send(&app, get(&format!("/r/{etoken}"))).await;
    assert_eq!(r.status, StatusCode::GONE, "expired link must be 410 even on a cache hit");

    let future = send(&app, json_req("POST", "/api/qr/create",
        json!({ "url": "https://future.com", "expires_at": "2099-01-01T00:00:00Z" }))).await;
    let ftoken = future.json()["token"].as_str().unwrap().to_string();
    let r = send(&app, get(&format!("/r/{ftoken}"))).await;
    assert_eq!(r.status, StatusCode::FOUND);
}

#[tokio::test]
async fn invalid_urls_are_rejected_400() {
    let app = test_app().await;

    // Blocklisted domain.
    let r = send(&app, json_req("POST", "/api/qr/create", json!({ "url": "https://evil.com/x" }))).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Disallowed scheme.
    let r = send(&app, json_req("POST", "/api/qr/create", json!({ "url": "ftp://example.com" }))).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

// ===========================================================================
// SQL-injection scenarios
//
// repo.rs uses parameterized queries (`?` placeholders + `.bind(..)`), so attacker
// input is treated as data, never SQL. These tests prove that empirically.
// ===========================================================================

/// A classic tautology in the token must NOT match every row; it should 404.
#[tokio::test]
async fn sqli_tautology_token_does_not_match() {
    let app = test_app().await;
    // Seed a real row so "OR '1'='1" would expose it if injection worked.
    create(&app, "https://secret.example.com").await;

    // token = ' OR '1'='1   (percent-encoded for a valid request line)
    let uri = "/r/%27%20OR%20%271%27%3D%271";
    let r = send(&app, get(uri)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "tautology token must not match any row");
    assert!(r.location().is_none(), "must not leak a redirect to the seeded URL");
}

/// A `DROP TABLE` payload in the token must be inert; the table survives.
#[tokio::test]
async fn sqli_drop_table_is_inert() {
    let app = test_app().await;

    // token = x'; DROP TABLE url_mappings;--   (percent-encoded)
    let uri = "/api/qr/x%27%3B%20DROP%20TABLE%20url_mappings%3B--";
    let r = send(&app, get(uri)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // If the table had been dropped, this create would 500. It must still succeed,
    // proving the payload never executed as SQL.
    let r = send(&app, json_req("POST", "/api/qr/create", json!({ "url": "https://after.com" }))).await;
    assert_eq!(r.status, StatusCode::OK, "url_mappings table must still exist");
}

/// SQL metacharacters inside the *URL* are stored and returned verbatim (bound as data),
/// and don't corrupt the table.
#[tokio::test]
async fn sqli_metachars_in_url_are_stored_as_data() {
    let app = test_app().await;

    let malicious = "https://example.com/path?q=%27%29%3B+DROP+TABLE+url_mappings%3B--";
    let token = create(&app, malicious).await;

    // Round-trips faithfully through the parameterized INSERT/SELECT.
    let r = send(&app, get(&format!("/api/qr/{token}"))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["original_url"], json!(malicious));

    // Table intact: another create still works.
    let r = send(&app, json_req("POST", "/api/qr/create", json!({ "url": "https://still-here.com" }))).await;
    assert_eq!(r.status, StatusCode::OK);
}

/// Injection attempt in the analytics token must simply find no scans (count 0), not error.
#[tokio::test]
async fn sqli_in_analytics_token_is_inert() {
    let app = test_app().await;
    // token = ' OR 1=1 --   (would sum ALL scans if injectable)
    let uri = "/api/qr/%27%20OR%201%3D1%20--/analytics";
    let r = send(&app, get(uri)).await;
    // The token doesn't exist -> 404 from the existence check (never reaches the count).
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}
