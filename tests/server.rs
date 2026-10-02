use std::net::{IpAddr, Ipv4Addr};

use simple_confidence_monitor::server::{Config, Server, StartError};

fn loopback(port: u16) -> Config {
    Config {
        bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        token: None,
        state_dir: None,
        name: None,
        mdns: false,
    }
}

async fn picker_status(server: &Server) -> reqwest::StatusCode {
    reqwest::get(format!("http://{}/", server.addr()))
        .await
        .expect("request")
        .status()
}

#[tokio::test]
async fn a_server_stops_and_starts_again_on_the_same_port() {
    let server = Server::start(loopback(0)).await.expect("start");
    let port = server.addr().port();
    assert_ne!(port, 0);
    assert!(picker_status(&server).await.is_success());

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(server.run_until(async move {
        let _ = stopped.await;
    }));
    stop.send(()).expect("send stop");
    running.await.expect("run_until");

    let again = Server::start(loopback(port)).await.expect("restart");
    assert!(picker_status(&again).await.is_success());
}

#[tokio::test]
async fn a_taken_port_is_a_bind_error() {
    let held = Server::start(loopback(0)).await.expect("start");
    let taken = held.addr().port();
    match Server::start(loopback(taken)).await {
        Err(StartError::Bind { addr, .. }) => assert_eq!(addr.port(), taken),
        Err(err) => panic!("expected a bind error, got {err}"),
        Ok(_) => panic!("expected a bind error, got a second server"),
    }
}

#[tokio::test]
async fn an_unwritable_state_dir_is_a_state_dir_error() {
    let file = std::env::temp_dir().join(format!("scm-not-a-dir-{}", std::process::id()));
    std::fs::write(&file, b"").expect("write file");
    let config = Config {
        state_dir: Some(file.join("rooms")),
        ..loopback(0)
    };
    let result = Server::start(config).await;
    let _ = std::fs::remove_file(&file);
    assert!(matches!(result, Err(StartError::StateDir { .. })));
}
