use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::PathBuf;

use clap::Parser;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::routes::router;
use tracing_subscriber::EnvFilter;

/// A speaker timer and confidence monitor served from one binary.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port to listen on.
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Address to bind. Defaults to every interface so the stage display can reach it.
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: IpAddr,

    /// Token required to open the operator console and to send commands.
    #[arg(short, long, env = "SCM_TOKEN")]
    token: Option<String>,

    /// Directory for room snapshots. Without it, state stays in memory.
    #[arg(short, long)]
    state_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    if args.token.is_none() {
        tracing::warn!("no --token given: anyone on this network can control every room");
    }

    let hub = Hub::new();
    let app = router(hub);
    let addr = SocketAddr::new(args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let host = advertised_host(args.bind);
    tracing::info!("listening on http://{addr}");
    tracing::info!("open http://{host}:{} to pick a room", args.port);

    axum::serve(listener, app).await?;
    Ok(())
}

/// Best-effort LAN address to print, so an operator can read a URL off the screen.
fn advertised_host(bind: IpAddr) -> String {
    if !bind.is_unspecified() {
        return bind.to_string();
    }
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "localhost".to_string())
}
