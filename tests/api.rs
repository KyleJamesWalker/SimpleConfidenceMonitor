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

    // A write brings a room into being. A read does not.
    for room in ["keynote", "breakout"] {
        client()
            .get(format!("http://{addr}/api/rooms/{room}/cmd?cmd=start"))
            .send()
            .await
            .unwrap();
    }

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

#[tokio::test]
async fn a_guarded_server_keeps_the_agenda_open() {
    let addr = guarded_server().await;
    let response = client()
        .get(format!("http://{addr}/keynote/agenda"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

async fn add_two_cues(addr: SocketAddr) {
    for (title, ms) in [("Welcome", 300_000), ("Keynote", 1_800_000)] {
        client()
            .post(format!("http://{addr}/api/rooms/keynote/cmd"))
            .json(&serde_json::json!({"cmd": "add_cue", "title": title, "duration_ms": ms}))
            .send()
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn the_rundown_exports_as_csv() {
    let addr = open_server().await;
    add_two_cues(addr).await;
    let response = client()
        .get(format!("http://{addr}/api/rooms/keynote/rundown.csv"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("csv")
    );
    let disposition = response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.contains("keynote-rundown.csv"),
        "got {disposition}"
    );
    let body = response.text().await.unwrap();
    assert!(
        body.starts_with("title,speaker,duration,notes\n"),
        "got {body}"
    );
    assert!(body.contains("Welcome,,5:00,"), "got {body}");
}

#[tokio::test]
async fn the_rundown_exports_as_json() {
    let addr = open_server().await;
    add_two_cues(addr).await;
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms/keynote/rundown.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["cues"].as_array().unwrap().len(), 2);
    assert_eq!(body["cues"][1]["title"], "Keynote");
    assert_eq!(body["cues"][1]["duration_ms"], 1_800_000);
}

#[tokio::test]
async fn a_csv_import_replaces_the_rundown() {
    let addr = open_server().await;
    add_two_cues(addr).await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/rundown"))
        .header("content-type", "text/csv")
        .body("title,speaker,duration\nPanel,Alice,20:00\n")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["rundown"]["cues"].as_array().unwrap().len(), 1);
    assert_eq!(body["rundown"]["cues"][0]["title"], "Panel");
    assert_eq!(body["rundown"]["cues"][0]["speaker"], "Alice");
}

#[tokio::test]
async fn a_json_import_replaces_the_rundown() {
    let addr = open_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/rundown"))
        .json(&serde_json::json!({"cues": [{"title": "Panel", "duration_ms": 60_000}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["rundown"]["cues"][0]["duration_ms"], 60_000);
}

#[tokio::test]
async fn an_exported_rundown_imports_again() {
    let addr = open_server().await;
    add_two_cues(addr).await;
    let csv = client()
        .get(format!("http://{addr}/api/rooms/keynote/rundown.csv"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let body: serde_json::Value = client()
        .post(format!("http://{addr}/api/rooms/breakout/rundown"))
        .header("content-type", "text/csv")
        .body(csv)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rundown"]["cues"].as_array().unwrap().len(), 2);
    assert_eq!(body["rundown"]["cues"][0]["title"], "Welcome");
}

#[tokio::test]
async fn a_broken_import_is_refused_and_keeps_the_rundown() {
    let addr = open_server().await;
    add_two_cues(addr).await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/rundown"))
        .header("content-type", "text/csv")
        .body("title,duration\nPanel,soon\n")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(response.text().await.unwrap().contains("line 2"));

    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms/keynote/rundown.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["cues"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn a_guarded_server_refuses_an_import_without_a_token() {
    let addr = guarded_server().await;
    let response = client()
        .post(format!("http://{addr}/api/rooms/keynote/rundown"))
        .header("content-type", "text/csv")
        .body("title,duration\nPanel,20\n")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn a_guarded_server_keeps_the_export_open() {
    let addr = guarded_server().await;
    let response = client()
        .get(format!("http://{addr}/api/rooms/keynote/rundown.csv"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

async fn get_cmd(addr: SocketAddr, query: &str) -> reqwest::Response {
    client()
        .get(format!("http://{addr}/api/rooms/keynote/cmd?{query}"))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn a_command_arrives_over_get() {
    let addr = open_server().await;
    let response = get_cmd(addr, "cmd=start").await;
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["timer"]["run"]["state"], "running");
    assert_eq!(body["rev"], 1);
}

#[tokio::test]
async fn a_get_command_reads_a_number_field() {
    let addr = open_server().await;
    let body: serde_json::Value = get_cmd(addr, "cmd=set_duration&ms=300000")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["timer"]["duration_ms"], 300_000);
}

#[tokio::test]
async fn a_get_command_reads_a_negative_number() {
    let addr = open_server().await;
    get_cmd(addr, "cmd=set_duration&ms=300000").await;
    let body: serde_json::Value = get_cmd(addr, "cmd=adjust&ms=-60000")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["timer"]["duration_ms"], 240_000);
}

#[tokio::test]
async fn a_get_command_reads_a_boolean_field() {
    let addr = open_server().await;
    let body: serde_json::Value = get_cmd(addr, "cmd=blackout&on=true")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["display"]["blackout"], true);
}

#[tokio::test]
async fn a_get_command_keeps_a_numeric_message_as_text() {
    let addr = open_server().await;
    let body: serde_json::Value = get_cmd(addr, "cmd=message&text=5&visible=true")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["message"]["text"], "5");
    assert_eq!(body["message"]["visible"], true);
}

#[tokio::test]
async fn a_get_command_reads_an_enum_field() {
    let addr = open_server().await;
    let body: serde_json::Value = get_cmd(addr, "cmd=set_mode&mode=count_up")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["timer"]["mode"], "count_up");
}

#[tokio::test]
async fn a_get_command_without_a_name_is_refused() {
    let addr = open_server().await;
    assert_eq!(get_cmd(addr, "ms=1000").await.status(), 400);
}

#[tokio::test]
async fn an_unknown_get_command_is_refused() {
    let addr = open_server().await;
    assert_eq!(get_cmd(addr, "cmd=explode").await.status(), 400);
}

#[tokio::test]
async fn a_guarded_server_gates_a_get_command() {
    let addr = guarded_server().await;
    assert_eq!(get_cmd(addr, "cmd=start").await.status(), 401);
    assert_eq!(get_cmd(addr, "cmd=start&token=s3cret").await.status(), 200);
}

#[tokio::test]
async fn deleting_a_room_takes_it_off_the_list() {
    let addr = open_server().await;
    get_cmd(addr, "cmd=start").await;
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rooms"], serde_json::json!(["keynote"]));

    let response = client()
        .delete(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["removed"], true);

    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rooms"], serde_json::json!([]));
}

#[tokio::test]
async fn deleting_a_room_that_is_not_there_says_so() {
    let addr = open_server().await;
    let body: serde_json::Value = client()
        .delete(format!("http://{addr}/api/rooms/ghost"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["removed"], false);
}

#[tokio::test]
async fn a_guarded_server_refuses_a_delete_without_a_token() {
    let addr = guarded_server().await;
    let response = client()
        .delete(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn a_deleted_room_comes_back_empty() {
    let addr = open_server().await;
    get_cmd(addr, "cmd=set_duration&ms=60000").await;
    client()
        .delete(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms/keynote"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["timer"]["duration_ms"], 900_000);
}

#[tokio::test]
async fn a_get_command_keeps_a_numeric_speaker_and_note_as_text() {
    let addr = open_server().await;
    let body: serde_json::Value = get_cmd(
        addr,
        "cmd=add_cue&title=Opening&speaker=1234&notes=0&duration_ms=60000",
    )
    .await
    .json()
    .await
    .unwrap();
    let cue = &body["rundown"]["cues"][0];
    assert_eq!(cue["speaker"], "1234");
    assert_eq!(cue["notes"], "0");
    assert_eq!(cue["title"], "Opening");
}

#[tokio::test]
async fn a_get_command_keeps_a_boolean_looking_label_as_text() {
    let addr = open_server().await;
    let body: serde_json::Value = get_cmd(addr, "cmd=aux_set&label=true")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["aux"]["label"], "true");
}

#[tokio::test]
async fn reading_a_room_does_not_create_it() {
    let addr = open_server().await;
    for path in [
        "/typo-viewer",
        "/typo-agenda/agenda",
        "/api/rooms/typo-state",
        "/api/rooms/typo-export/rundown.csv",
    ] {
        let response = client()
            .get(format!("http://{addr}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "{path} should still serve");
    }

    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body["rooms"],
        serde_json::json!([]),
        "a read must not litter the room list"
    );
}

#[tokio::test]
async fn reading_a_room_that_is_not_there_returns_the_defaults() {
    let addr = open_server().await;
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms/ghost"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rev"], 0);
    assert_eq!(body["timer"]["duration_ms"], 900_000);
    assert_eq!(body["viewers"], 0);
}

#[tokio::test]
async fn a_write_still_creates_the_room() {
    let addr = open_server().await;
    get_cmd(addr, "cmd=start").await;
    let body: serde_json::Value = client()
        .get(format!("http://{addr}/api/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["rooms"], serde_json::json!(["keynote"]));
}
