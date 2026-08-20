use std::net::SocketAddr;

use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::routes::{AppState, router};

async fn serve(state: AppState) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(state);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn open_server() -> SocketAddr {
    serve(AppState::open(Hub::new())).await
}

async fn guarded_server() -> SocketAddr {
    serve(AppState::guarded(Hub::new(), "s3cret")).await
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

#[tokio::test]
async fn the_state_endpoint_returns_the_room() {
    let addr = open_server().await;
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rev"], 0);
    assert_eq!(body["timer"]["duration_ms"], 900_000);
    assert!(body["server_time_ms"].as_u64().unwrap() > 1_700_000_000_000);
}

#[tokio::test]
async fn a_posted_command_changes_the_room() {
    let addr = open_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/cmd"))
        .json(&serde_json::json!({"cmd": "set_duration", "ms": 300_000}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["rev"], 1);
    assert_eq!(body["timer"]["duration_ms"], 300_000);
}

#[tokio::test]
async fn an_unknown_command_is_refused_and_changes_nothing() {
    let addr = open_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/cmd"))
        .json(&serde_json::json!({"cmd": "explode"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rev"], 0);
}

#[tokio::test]
async fn a_command_for_an_invalid_room_name_is_refused() {
    let addr = open_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/a%20b/cmd"))
        .json(&serde_json::json!({"cmd": "start"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn a_guarded_server_refuses_a_command_without_a_token() {
    let addr = guarded_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/cmd"))
        .json(&serde_json::json!({"cmd": "start"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn a_guarded_server_accepts_a_command_with_a_bearer_token() {
    let addr = guarded_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/cmd"))
        .bearer_auth("s3cret")
        .json(&serde_json::json!({"cmd": "start"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn a_guarded_server_keeps_the_viewer_and_the_state_open() {
    let addr = guarded_server().await;
    for path in ["/keynote", "/api/rooms/keynote", "/healthz"] {
        let response = client()
            .get(format!("http://{addr}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "{path} should stay open");
    }
}

#[tokio::test]
async fn a_guarded_server_refuses_the_console_without_a_token() {
    let addr = guarded_server().await;
    let response = client()
        .get(format!("http://{addr}/keynote/edit"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn the_console_stores_a_cookie_when_the_token_arrives_in_the_url() {
    let addr = guarded_server().await;
    let response = client()
        .get(format!("http://{addr}/keynote/edit?token=s3cret"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response
        .headers()
        .get("set-cookie")
        .expect("a cookie should come back")
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("scm_token=s3cret"), "got {cookie}");
    assert!(cookie.contains("HttpOnly"), "got {cookie}");
}

#[tokio::test]
async fn the_console_accepts_the_stored_cookie() {
    let addr = guarded_server().await;
    let response = client()
        .get(format!("http://{addr}/keynote/edit"))
        .header("cookie", "scm_token=s3cret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn a_guarded_server_refuses_an_edit_socket_without_a_token() {
    let addr = guarded_server().await;
    let url = format!("ws://{addr}/api/rooms/keynote/ws?role=edit");
    assert!(tokio_tungstenite::connect_async(url).await.is_err());

    let url = format!("ws://{addr}/api/rooms/keynote/ws?role=view");
    assert!(tokio_tungstenite::connect_async(url).await.is_ok());
}

#[tokio::test]
async fn a_guarded_server_accepts_an_edit_socket_with_the_token() {
    let addr = guarded_server().await;
    let url = format!("ws://{addr}/api/rooms/keynote/ws?role=edit&token=s3cret");
    assert!(tokio_tungstenite::connect_async(url).await.is_ok());
}

#[tokio::test]
async fn the_qr_endpoint_returns_an_svg() {
    let addr = open_server().await;
    let response = client()
        .get(format!(
            "http://{addr}/api/qr?text=http://192.168.1.20:8080/keynote"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("svg")
    );
    assert!(response.text().await.unwrap().contains("<svg"));
}

#[tokio::test]
async fn the_qr_endpoint_refuses_an_empty_request() {
    let addr = open_server().await;
    let response = client()
        .get(format!("http://{addr}/api/qr"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn the_room_list_names_the_live_rooms() {
    let addr = open_server().await;
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rooms"], serde_json::json!([]));

    client()
        .get(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap();
    client()
        .get(format!("http://{addr}/breakout"))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rooms"], serde_json::json!(["breakout", "keynote"]));
}
