use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::routes::{AppState, router};
use tokio_tungstenite::tungstenite::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn serve() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(AppState::open(Hub::new()));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn connect(addr: SocketAddr, room: &str, role: &str) -> Socket {
    let url = format!("ws://{addr}/api/rooms/{room}/ws?role={role}");
    tokio_tungstenite::connect_async(url).await.unwrap().0
}

/// Reads frames until one of the wanted type arrives.
async fn next_frame(socket: &mut Socket, kind: &str) -> Value {
    for _ in 0..10 {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
            .await
            .expect("frame should arrive")
            .expect("stream should stay open")
            .expect("frame should decode");
        if let Message::Text(text) = msg {
            let value: Value = serde_json::from_str(&text).unwrap();
            if value["type"] == kind {
                return value;
            }
        }
    }
    panic!("no {kind} frame arrived");
}

async fn send(socket: &mut Socket, json: &str) {
    socket.send(Message::text(json.to_string())).await.unwrap();
}

#[tokio::test]
async fn a_viewer_receives_the_state_on_connect() {
    let addr = serve().await;
    let mut viewer = connect(addr, "keynote", "view").await;
    let frame = next_frame(&mut viewer, "state").await;
    assert_eq!(frame["rev"], 0);
    assert_eq!(frame["timer"]["duration_ms"], 900_000);
    assert!(frame["server_time_ms"].as_u64().unwrap() > 1_700_000_000_000);
}

#[tokio::test]
async fn an_editor_command_reaches_a_viewer() {
    let addr = serve().await;
    let mut editor = connect(addr, "keynote", "edit").await;
    let mut viewer = connect(addr, "keynote", "view").await;
    next_frame(&mut viewer, "state").await;
    send(&mut editor, r#"{"cmd":"start"}"#).await;

    let frame = next_frame(&mut viewer, "state").await;
    assert_eq!(frame["rev"], 1);
    assert_eq!(frame["timer"]["run"]["state"], "running");
}

#[tokio::test]
async fn a_command_only_reaches_the_room_it_names() {
    let addr = serve().await;
    let mut other = connect(addr, "breakout", "view").await;
    let mut editor = connect(addr, "keynote", "edit").await;
    next_frame(&mut other, "state").await;
    send(&mut editor, r#"{"cmd":"start"}"#).await;
    next_frame(&mut editor, "state").await;

    let timeout = tokio::time::timeout(std::time::Duration::from_millis(300), other.next()).await;
    assert!(timeout.is_err(), "breakout should hear nothing");
}

#[tokio::test]
async fn a_ping_returns_a_pong_carrying_both_clocks() {
    let addr = serve().await;
    let mut editor = connect(addr, "keynote", "edit").await;
    send(&mut editor, r#"{"cmd":"ping","client_time_ms":1234}"#).await;
    let frame = next_frame(&mut editor, "pong").await;
    assert_eq!(frame["client_time_ms"], 1234);
    assert!(frame["server_time_ms"].as_u64().unwrap() > 1_700_000_000_000);
}

#[tokio::test]
async fn a_viewer_may_not_send_a_command() {
    let addr = serve().await;
    let mut viewer = connect(addr, "keynote", "view").await;
    next_frame(&mut viewer, "state").await;
    send(&mut viewer, r#"{"cmd":"start"}"#).await;

    let frame = next_frame(&mut viewer, "error").await;
    assert!(
        frame["message"].as_str().unwrap().contains("read-only"),
        "got {frame}"
    );
}

#[tokio::test]
async fn a_bad_frame_returns_an_error_and_keeps_the_socket_open() {
    let addr = serve().await;
    let mut editor = connect(addr, "keynote", "edit").await;
    next_frame(&mut editor, "state").await;
    send(&mut editor, "not json").await;
    next_frame(&mut editor, "error").await;

    send(&mut editor, r#"{"cmd":"start"}"#).await;
    assert_eq!(next_frame(&mut editor, "state").await["rev"], 1);
}

#[tokio::test]
async fn the_state_frame_counts_the_connected_clients() {
    let addr = serve().await;
    let mut editor = connect(addr, "keynote", "edit").await;
    next_frame(&mut editor, "state").await;
    let mut viewer = connect(addr, "keynote", "view").await;
    next_frame(&mut viewer, "state").await;

    let frame = next_frame(&mut editor, "state").await;
    assert_eq!(frame["viewers"], 1);
    assert_eq!(frame["editors"], 1);

    drop(viewer);
    let frame = next_frame(&mut editor, "state").await;
    assert_eq!(frame["viewers"], 0);
}

#[tokio::test]
async fn an_invalid_room_name_is_refused() {
    let addr = serve().await;
    let url = format!("ws://{addr}/api/rooms/a%20b/ws?role=edit");
    assert!(tokio_tungstenite::connect_async(url).await.is_err());
}

#[tokio::test]
async fn a_message_reaches_a_viewer_with_its_tone() {
    let addr = serve().await;
    let mut editor = connect(addr, "keynote", "edit").await;
    let mut viewer = connect(addr, "keynote", "view").await;
    next_frame(&mut viewer, "state").await;
    send(
        &mut editor,
        r#"{"cmd":"message","text":"Wrap up","tone":"alert","visible":true}"#,
    )
    .await;

    let frame = next_frame(&mut viewer, "state").await;
    assert_eq!(frame["message"]["text"], "Wrap up");
    assert_eq!(frame["message"]["tone"], "alert");
    assert_eq!(frame["message"]["visible"], true);
}

#[tokio::test]
async fn the_state_frame_carries_the_display_settings() {
    let addr = serve().await;
    let mut viewer = connect(addr, "keynote", "view").await;
    let frame = next_frame(&mut viewer, "state").await;
    assert_eq!(frame["display"]["scale"], 100);
    assert_eq!(frame["display"]["show_clock"], true);
    assert_eq!(frame["display"]["blackout"], false);
    assert_eq!(frame["display"]["title"], "");
}

#[tokio::test]
async fn deleting_a_room_closes_the_sockets_it_holds() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hub = Hub::new();
    let app = router(AppState::open(hub.clone()));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let mut editor = connect(addr, "keynote", "edit").await;
    next_frame(&mut editor, "state").await;

    hub.remove(&simple_confidence_monitor::room::RoomName::parse("keynote").unwrap());

    // The socket must end rather than keep driving a room nobody can reach.
    let closed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match editor.next().await {
                None => return true,
                Some(Err(_)) => return true,
                Some(Ok(Message::Close(_))) => return true,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await;
    assert_eq!(closed, Ok(true), "the socket should close with the room");
}
