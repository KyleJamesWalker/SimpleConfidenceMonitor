use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use simple_confidence_monitor::auth::Auth;
use simple_confidence_monitor::autopilot::SCAN_INTERVAL;
use simple_confidence_monitor::discovery;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::persist::{Snapshots, Store};
use simple_confidence_monitor::routes::{AppState, router};
use tracing_subscriber::EnvFilter;

/// How long a room settles before its snapshot is written.
const SNAPSHOT_DEBOUNCE: Duration = Duration::from_secs(1);

/// A speaker timer and confidence monitor served from one binary.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port to listen on.
    #[arg(short, long, env = "SCM_PORT", default_value_t = 8080)]
    port: u16,

    /// Address to bind. Defaults to every interface so the stage display can reach it.
    #[arg(short, long, env = "SCM_BIND", default_value = "0.0.0.0")]
    bind: IpAddr,

    /// Token required to open the operator console and to send commands.
    #[arg(short, long, env = "SCM_TOKEN")]
    token: Option<String>,

    /// Directory for room snapshots. Without it, state stays in memory.
    #[arg(short, long, env = "SCM_STATE_DIR")]
    state_dir: Option<PathBuf>,

    /// Name to advertise on the local network. Defaults to the port.
    #[arg(long, env = "SCM_NAME")]
    name: Option<String>,

    /// Advertise the server over mDNS, so a phone can find it by name.
    #[arg(long)]
    mdns: bool,
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
    let auth = Arc::new(match &args.token {
        Some(token) => Auth::with_token(token.clone()),
        None => Auth::open(),
    });

    let opened = match &args.state_dir {
        Some(dir) => {
            let store = open_store(dir);
            let restored = store.load_all();
            Some((Arc::new(Snapshots::new(store)), restored))
        }
        None => None,
    };
    let snapshots = opened.as_ref().map(|(snapshots, _)| snapshots.clone());
    let hub = match &snapshots {
        Some(snapshots) => Hub::with_snapshots(snapshots.clone()),
        None => Hub::new(),
    };
    if let Some((snapshots, restored)) = opened {
        if !restored.is_empty() {
            tracing::info!(
                "restored {} room(s) from {:?}",
                restored.len(),
                args.state_dir
            );
        }
        hub.restore(restored);
        let flusher_hub = hub.clone();
        let flusher = snapshots;
        tokio::spawn(async move { flusher.run(&flusher_hub, SNAPSHOT_DEBOUNCE).await });
    }

    {
        let pilot_hub = hub.clone();
        tokio::spawn(async move {
            simple_confidence_monitor::autopilot::run(&pilot_hub, SCAN_INTERVAL).await
        });
    }

    let app = router(AppState::new(hub, auth));
    let addr = SocketAddr::new(args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let host = advertised_host(args.bind);
    tracing::info!("listening on http://{addr}");
    tracing::info!("open http://{host}:{} to pick a room", args.port);

    // Held for the life of the process. Dropping it withdraws the service.
    let _advertisement = match args.mdns {
        false => None,
        true => match discovery::advertise(args.port, args.name.as_deref()) {
            Ok(advertisement) => {
                tracing::info!("advertised on this network as {}", advertisement.fullname());
                Some(advertisement)
            }
            Err(err) => {
                tracing::warn!("could not advertise over mDNS: {err}");
                None
            }
        },
    };

    axum::serve(listener, app).await?;
    Ok(())
}

/// Best-effort LAN address to print, so an operator can read a URL off the screen.
/// Snapshots that never land are worse than no snapshots, so refuse to start.
fn open_store(dir: &Path) -> Store {
    match Store::new(dir) {
        Ok(store) => store,
        Err(err) => {
            tracing::error!(
                "cannot write to the state directory {}: {err}",
                dir.display()
            );
            tracing::error!(
                "a Docker bind mount keeps the host ownership: chown it to the user this container runs as, or set `user:` to match the directory"
            );
            std::process::exit(1);
        }
    }
}

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
