use mdns_sd::{ServiceDaemon, ServiceInfo};

/// The service type this server advertises.
pub const SERVICE_TYPE: &str = "_scm._tcp.local.";

/// Longest instance name a DNS label holds.
const MAX_NAME_LEN: usize = 63;

/// The name a browser shows. A dot would split the DNS label, so it goes.
pub fn instance_name(custom: Option<&str>, port: u16) -> String {
    let cleaned: String = custom
        .unwrap_or_default()
        .trim()
        .chars()
        .map(|c| if c == '.' { '-' } else { c })
        .take(MAX_NAME_LEN)
        .collect();
    if cleaned.is_empty() {
        format!("confidence-monitor-{port}")
    } else {
        cleaned
    }
}

/// A live advertisement. Dropping it withdraws the service.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Announces the server on the local network, so nobody reads an IP address
/// aloud. A venue network that blocks multicast returns an error.
pub fn advertise(port: u16, name: Option<&str>) -> Result<Advertisement, String> {
    let daemon = ServiceDaemon::new().map_err(|err| err.to_string())?;
    let instance = instance_name(name, port);
    let host = format!("{instance}.local.");
    let service = ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &host,
        (),
        port,
        &[("path", "/")][..],
    )
    .map_err(|err| err.to_string())?
    .enable_addr_auto();

    let fullname = service.get_fullname().to_string();
    daemon.register(service).map_err(|err| err.to_string())?;
    Ok(Advertisement { daemon, fullname })
}
