use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;
use simple_confidence_monitor::server::{Config, Server, StartError};
use tracing_subscriber::EnvFilter;

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

    let config = Config {
        bind: args.bind,
        port: args.port,
        token: args.token,
        state_dir: args.state_dir,
        name: args.name,
        mdns: args.mdns,
    };
    let server = match Server::start(config).await {
        Ok(server) => server,
        Err(err @ StartError::StateDir { .. }) => {
            tracing::error!("{err}");
            tracing::error!(
                "a Docker bind mount keeps the host ownership: chown it to the user this container runs as, or set `user:` to match the directory"
            );
            std::process::exit(1);
        }
        Err(err) => return Err(err.into()),
    };
    server.run_until(shutdown_signal()).await;
    Ok(())
}

/// Ctrl-C, or the TERM a container runtime sends.
async fn shutdown_signal() {
    let interrupt = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::warn!("could not listen for Ctrl-C: {err}");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::warn!("could not listen for SIGTERM: {err}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => {}
        _ = terminate => {}
    }
}
