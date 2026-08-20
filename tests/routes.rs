use axum::body::Body;
use axum::http::{Request, StatusCode};
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::routes::{AppState, router};
use tower::ServiceExt;

async fn get(uri: &str) -> (StatusCode, String) {
    let app = router(AppState::open(Hub::new()));
    let res = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

#[tokio::test]
async fn healthz_reports_ok() {
    let (status, body) = get("/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn viewer_route_serves_html() {
    let (status, body) = get("/keynote").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn viewer_route_rejects_an_invalid_room_name() {
    let (status, _) = get("/a%20b").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn viewer_route_creates_the_room() {
    let hub = Hub::new();
    let app = router(AppState::open(hub.clone()));
    let res = app
        .oneshot(
            Request::builder()
                .uri("/keynote")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(hub.room_count(), 1);
}

#[tokio::test]
async fn missing_asset_returns_not_found() {
    let (status, _) = get("/assets/nope.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
