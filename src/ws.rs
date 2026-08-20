use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};

use crate::clock::now_ms;
use crate::room::Room;
use crate::wire::{ClientMsg, ServerMsg, parse_client_msg};

/// Resend the state on this interval. It doubles as a clock resynchronization.
const KEEPALIVE: Duration = Duration::from_secs(15);

/// A socket role. Only an editor may send commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    View,
    Edit,
}

impl Role {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("edit") => Self::Edit,
            _ => Self::View,
        }
    }

    fn is_editor(self) -> bool {
        self == Self::Edit
    }
}

/// Keeps the room client count right even when a socket drops mid-show.
struct Membership {
    room: Arc<Room>,
    editor: bool,
}

impl Membership {
    fn join(room: Arc<Room>, role: Role) -> Self {
        let editor = role.is_editor();
        room.client_joined(editor);
        Self { room, editor }
    }
}

impl Drop for Membership {
    fn drop(&mut self) {
        self.room.client_left(self.editor);
    }
}

pub async fn serve_socket(socket: WebSocket, room: Arc<Room>, role: Role) {
    let (mut sink, mut stream) = socket.split();
    // Subscribe before joining, so this client also receives its own join frame.
    let mut frames = room.subscribe();
    let _membership = Membership::join(room.clone(), role);
    let mut keepalive = tokio::time::interval(KEEPALIVE);
    keepalive.tick().await;

    loop {
        tokio::select! {
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { break };
                let Some(reply) = handle(&room, role, message) else { continue };
                if sink.send(Message::text(reply)).await.is_err() {
                    break;
                }
            }
            frame = frames.recv() => {
                let text = match frame {
                    Ok(text) => text,
                    // A client that fell behind gets the current state instead of history.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => room.frame(),
                    Err(_) => break,
                };
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
                if room.is_closed() {
                    break;
                }
            }
            _ = keepalive.tick() => {
                if sink.send(Message::text(room.frame())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Returns the frame to send back, if the message calls for one.
fn handle(room: &Arc<Room>, role: Role, message: Message) -> Option<String> {
    let text = match message {
        Message::Text(text) => text,
        Message::Close(_) => return None,
        _ => return None,
    };

    match parse_client_msg(&text) {
        Ok(ClientMsg::Ping { client_time_ms }) => Some(encode(&ServerMsg::Pong {
            client_time_ms,
            server_time_ms: now_ms(),
        })),
        Ok(ClientMsg::Cmd(cmd)) if role.is_editor() => {
            room.apply(&cmd, now_ms());
            None
        }
        Ok(ClientMsg::Cmd(_)) => Some(error(
            "this socket is read-only, open the console to control the room",
        )),
        Err(message) => Some(error(&message)),
    }
}

fn error(message: &str) -> String {
    encode(&ServerMsg::Error {
        message: message.to_string(),
    })
}

fn encode(msg: &ServerMsg<'_>) -> String {
    serde_json::to_string(msg).expect("frame serializes")
}
