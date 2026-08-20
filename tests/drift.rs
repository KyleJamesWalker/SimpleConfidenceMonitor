//! Measures how far a client's estimate of the server clock strays over a run.
//! The sync design rests on that estimate, so this reports the number rather
//! than only asserting a bound.
//!
//! Run with: cargo test --test drift -- --ignored --nocapture

use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::routes::{AppState, router};
use tokio_tungstenite::tungstenite::Message;

/// Samples to take, and the gap between them.
const SAMPLES: usize = 40;
const GAP: Duration = Duration::from_millis(250);

/// Loopback should hold well inside this. A venue network will not, and that is
/// what the number in the output is for.
const MAX_ABS_OFFSET_MS: f64 = 50.0;
const MAX_SPREAD_MS: f64 = 50.0;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64
}

async fn serve() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(AppState::open(Hub::new()));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
#[ignore = "takes ten seconds and measures rather than asserts behavior"]
async fn the_clock_offset_stays_steady_over_a_run() {
    let addr = serve().await;
    let url = format!("ws://{addr}/api/rooms/keynote/ws?role=edit");
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    socket
        .send(Message::text(r#"{"cmd":"start"}"#.to_string()))
        .await
        .unwrap();

    let mut offsets: Vec<f64> = Vec::new();
    for _ in 0..SAMPLES {
        let sent = now_ms();
        socket
            .send(Message::text(format!(
                r#"{{"cmd":"ping","client_time_ms":{sent}}}"#
            )))
            .await
            .unwrap();

        // Read until the pong. State frames arrive on the same socket.
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
                .await
                .expect("a pong should arrive")
                .expect("the socket should stay open")
                .expect("the frame should decode");
            if let Message::Text(text) = message {
                let frame: Value = serde_json::from_str(&text).unwrap();
                if frame["type"] == "pong" {
                    let received = now_ms() as f64;
                    let server = frame["server_time_ms"].as_f64().unwrap();
                    let rtt = received - sent as f64;
                    offsets.push(server + rtt / 2.0 - received);
                    break;
                }
            }
        }
        tokio::time::sleep(GAP).await;
    }

    let count = offsets.len() as f64;
    let mean = offsets.iter().sum::<f64>() / count;
    let min = offsets.iter().cloned().fold(f64::MAX, f64::min);
    let max = offsets.iter().cloned().fold(f64::MIN, f64::max);
    let spread = max - min;
    let worst = offsets
        .iter()
        .cloned()
        .fold(0.0_f64, |acc, v| acc.max(v.abs()));

    // First against last, which is drift rather than jitter.
    let drift = offsets.last().unwrap() - offsets.first().unwrap();

    println!(
        "offset over {SAMPLES} samples: mean {mean:.2}ms, min {min:.2}ms, max {max:.2}ms, spread {spread:.2}ms, drift {drift:.2}ms"
    );

    assert!(
        worst <= MAX_ABS_OFFSET_MS,
        "worst offset {worst:.2}ms is over {MAX_ABS_OFFSET_MS}ms on loopback"
    );
    assert!(
        spread <= MAX_SPREAD_MS,
        "spread {spread:.2}ms is over {MAX_SPREAD_MS}ms on loopback"
    );
}
