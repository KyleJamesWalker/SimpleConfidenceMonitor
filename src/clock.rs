use std::time::{SystemTime, UNIX_EPOCH};

/// Wall clock in epoch milliseconds. Every timer anchor and state frame uses this.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
