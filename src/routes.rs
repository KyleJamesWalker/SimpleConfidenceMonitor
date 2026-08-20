use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::assets::Web;
use crate::hub::Hub;
use crate::room::RoomName;

pub fn router(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/", get(picker))
        .route("/healthz", get(healthz))
        .route("/assets/{*path}", get(asset))
        .route("/{room}", get(viewer))
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
