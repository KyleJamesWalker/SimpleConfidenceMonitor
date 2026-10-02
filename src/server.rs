use std::fmt;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::auth::Auth;
use crate::autopilot::SCAN_INTERVAL;
use crate::discovery::{self, Advertisement};
use crate::hub::Hub;
use crate::persist::{Snapshots, Store};
use crate::routes::{AppState, router};

/// How long a room settles before its snapshot is written.
const SNAPSHOT_DEBOUNCE: Duration = Duration::from_secs(1);

/// How long a connection has to finish once the server stops listening.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// How long the closing sockets have to flush their close frames.
const SOCKET_DRAIN: Duration = Duration::from_millis(100);

/// Everything a binary decides before the server starts.
#[derive(Clone, Debug)]
pub struct Config {
    pub bind: IpAddr,
    pub port: u16,
    pub token: Option<String>,
    pub state_dir: Option<PathBuf>,
    pub name: Option<String>,
    pub mdns: bool,
}

#[derive(Debug)]
pub enum StartError {
    /// Snapshots that never land are worse than no snapshots, so this is fatal.
    StateDir {
        dir: PathBuf,
        source: io::Error,
    },
    Bind {
        addr: SocketAddr,
        source: io::Error,
    },
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateDir { dir, source } => {
                write!(
                    f,
                    "cannot write to the state directory {}: {source}",
                    dir.display()
                )
            }
            Self::Bind { addr, source } => write!(f, "cannot listen on {addr}: {source}"),
        }
    }
}

impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StateDir { source, .. } | Self::Bind { source, .. } => Some(source),
        }
    }
}

/// A listening server and the tasks that keep its rooms alive.
pub struct Server {
    addr: SocketAddr,
    hub: Arc<Hub>,
    snapshots: Option<Arc<Snapshots>>,
    serving: JoinHandle<io::Result<()>>,
    stop: oneshot::Sender<()>,
    background: Vec<JoinHandle<()>>,
    advertisement: Option<Advertisement>,
}

impl Server {
    /// Restores rooms, binds the listener and starts serving.
    pub async fn start(config: Config) -> Result<Self, StartError> {
        if config.token.is_none() {
            tracing::warn!("no token given: anyone on this network can control every room");
        }
        let auth = Arc::new(match &config.token {
            Some(token) => Auth::with_token(token.clone()),
            None => Auth::open(),
        });

        let mut background = Vec::new();
        let (hub, snapshots) = match &config.state_dir {
            Some(dir) => {
                let store = Store::new(dir).map_err(|source| StartError::StateDir {
                    dir: dir.clone(),
                    source,
                })?;
                let restored = store.load_all();
                let snapshots = Arc::new(Snapshots::new(store));
                let hub = Hub::with_snapshots(snapshots.clone());
                if !restored.is_empty() {
                    tracing::info!("restored {} room(s) from {}", restored.len(), dir.display());
                }
                hub.restore(restored);
                let flusher_hub = hub.clone();
                let flusher = snapshots.clone();
                background.push(tokio::spawn(async move {
                    flusher.run(&flusher_hub, SNAPSHOT_DEBOUNCE).await
                }));
                (hub, Some(snapshots))
            }
            None => (Hub::new(), None),
        };

        {
            let pilot_hub = hub.clone();
            background.push(tokio::spawn(async move {
                crate::autopilot::run(&pilot_hub, SCAN_INTERVAL).await
            }));
        }

        let requested = SocketAddr::new(config.bind, config.port);
        let listener = match tokio::net::TcpListener::bind(requested).await {
            Ok(listener) => listener,
            Err(source) => {
                background.iter().for_each(JoinHandle::abort);
                return Err(StartError::Bind {
                    addr: requested,
                    source,
                });
            }
        };
        let addr = listener.local_addr().unwrap_or(requested);

        tracing::info!("listening on http://{addr}");
        tracing::info!(
            "open http://{}:{} to pick a room",
            advertised_host(config.bind),
            addr.port()
        );

        let advertisement = match config.mdns {
            false => None,
            true => match discovery::advertise(addr.port(), config.name.as_deref()) {
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

        let app = router(AppState::new(hub.clone(), auth));
        let (stop, stopped) = oneshot::channel::<()>();
        let serving = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = stopped.await;
                })
                .await
        });

        Ok(Self {
            addr,
            hub,
            snapshots,
            serving,
            stop,
            background,
            advertisement,
        })
    }

    /// The address the listener holds, with the real port when 0 was asked for.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn hub(&self) -> Arc<Hub> {
        self.hub.clone()
    }

    /// The mDNS name, when the advertisement went out.
    pub fn advertised_as(&self) -> Option<&str> {
        self.advertisement.as_ref().map(Advertisement::fullname)
    }

    /// Serves until `shutdown` resolves or the accept loop ends, then closes
    /// every socket and writes whatever state is still pending.
    pub async fn run_until(self, shutdown: impl Future<Output = ()>) {
        let Self {
            hub,
            snapshots,
            serving,
            stop,
            background,
            advertisement,
            ..
        } = self;

        let mut serving = std::pin::pin!(serving);
        tokio::select! {
            joined = &mut serving => report(joined),
            () = shutdown => {
                tracing::info!("stopping");
                // Before the rooms close, or the writer would take a closing room
                // for a deleted one and skip it.
                write_pending(&snapshots, &hub);
                // Every socket outlives any request, so end them rather than
                // waiting on connections that stay open for as long as the show.
                hub.close_all();
                // A beat for those close frames to reach the wire.
                tokio::time::sleep(SOCKET_DRAIN).await;
                let _ = stop.send(());
                match tokio::time::timeout(SHUTDOWN_GRACE, serving).await {
                    Ok(joined) => report(joined),
                    Err(_) => tracing::warn!("a connection did not close within the grace period"),
                }
            }
        }
        write_pending(&snapshots, &hub);
        background.iter().for_each(JoinHandle::abort);
        drop(advertisement);
    }
}

/// Writes whatever is still inside the debounce window. A stop costs no state.
fn write_pending(snapshots: &Option<Arc<Snapshots>>, hub: &Hub) {
    let Some(snapshots) = snapshots else { return };
    let pending = snapshots.pending();
    snapshots.flush(hub);
    if pending > 0 {
        tracing::info!("wrote {pending} snapshot(s) on the way out");
    }
}

/// The accept loop is meant to outlive everything else, so say so when it does
/// not. There is nothing left to serve either way, and state still gets written.
fn report(joined: Result<io::Result<()>, tokio::task::JoinError>) {
    match joined {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::error!("stopped serving: {err}"),
        Err(err) => tracing::error!("the server task ended badly: {err}"),
    }
}

/// Best-effort LAN address to print, so an operator can read a URL off the screen.
pub fn advertised_host(bind: IpAddr) -> String {
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
