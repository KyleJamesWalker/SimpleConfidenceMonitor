use serde::{Deserialize, Serialize};

use crate::room::{Command, RoomState};

/// One frame from a client. Every client message carries a `cmd` field.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientMsg {
    /// Clock offset probe. The server echoes it with its own time.
    Ping {
        client_time_ms: u64,
    },
    Cmd(Command),
}

#[derive(Debug, Deserialize)]
struct PingFrame {
    client_time_ms: u64,
}

/// Server to client. Flattened so the browser reads `rev` and `timer` at the top level.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg<'a> {
    State {
        server_time_ms: u64,
        viewers: usize,
        editors: usize,
        #[serde(flatten)]
        state: &'a RoomState,
    },
    Pong {
        client_time_ms: u64,
        server_time_ms: u64,
    },
    Error {
        message: String,
    },
}

/// Reads a client frame. Ping is separate from Command so one envelope serves both.
pub fn parse_client_msg(text: &str) -> Result<ClientMsg, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("frame is not JSON: {err}"))?;
    let name = value
        .get("cmd")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "frame has no cmd field".to_string())?;

    if name == "ping" {
        let ping: PingFrame =
            serde_json::from_value(value).map_err(|err| format!("bad ping: {err}"))?;
        return Ok(ClientMsg::Ping {
            client_time_ms: ping.client_time_ms,
        });
    }

    let name = name.to_string();
    serde_json::from_value::<Command>(value)
        .map(ClientMsg::Cmd)
        .map_err(|err| format!("bad command {name}: {err}"))
}
