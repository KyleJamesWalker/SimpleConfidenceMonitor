use std::sync::Arc;

use std::collections::HashMap;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::assets::Web;
use crate::hub::Hub;
use crate::room::RoomName;
use crate::ws::{Role, serve_socket};

pub fn router(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/", get(picker))
        .route("/healthz", get(healthz))
        .route("/assets/{*path}", get(asset))
        .route("/api/rooms/{room}/ws", get(socket))
        .route("/{room}", get(viewer))
        .route("/{room}/edit", get(console))
        .with_state(hub)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn picker() -> Response {
    page("picker.html")
}

async fn viewer(State(hub): State<Arc<Hub>>, Path(room): Path<String>) -> Response {
    match RoomName::parse(&room) {
        Ok(name) => {
            hub.get_or_create(&name);
            page("viewer.html")
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn console(State(hub): State<Arc<Hub>>, Path(room): Path<String>) -> Response {
    match RoomName::parse(&room) {
        Ok(name) => {
            hub.get_or_create(&name);
            page("console.html")
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn socket(
    State(hub): State<Arc<Hub>>,
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let name = match RoomName::parse(&room) {
        Ok(name) => name,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let room = hub.get_or_create(&name);
    let role = Role::parse(params.get("role").map(String::as_str));
    upgrade.on_upgrade(move |socket| serve_socket(socket, room, role))
}

async fn asset(Path(path): Path<String>) -> Response {
    embedded(&path)
}

/// Serves an embedded HTML page, or 500 when the build lost the asset.
fn page(name: &str) -> Response {
    match Web::get(name) {
        Some(file) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            file.data.into_owned(),
        )
            .into_response(),
        None => (StatusCode::INTERNAL_SERVER_ERROR, format!("missing {name}")).into_response(),
    }
}

fn embedded(path: &str) -> Response {
    match Web::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
