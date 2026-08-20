use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use qrcode::QrCode;
use qrcode::render::svg;

use crate::assets::Web;
use crate::auth::{Auth, COOKIE, Outcome};
use crate::hub::Hub;
use crate::room::{Command, CueDraft, RoomName};
use crate::rundown_io::{parse_csv, to_csv};
use crate::ws::{Role, serve_socket};

/// Longest text the QR endpoint will encode. A room URL is far shorter.
const MAX_QR_LEN: usize = 512;

#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<Hub>,
    pub auth: Arc<Auth>,
}

impl AppState {
    pub fn open(hub: Arc<Hub>) -> Self {
        Self {
            hub,
            auth: Arc::new(Auth::open()),
        }
    }

    pub fn guarded(hub: Arc<Hub>, token: impl Into<String>) -> Self {
        Self {
            hub,
            auth: Arc::new(Auth::with_token(token)),
        }
    }

    pub fn new(hub: Arc<Hub>, auth: Arc<Auth>) -> Self {
        Self { hub, auth }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(picker))
        .route("/healthz", get(healthz))
        .route("/assets/{*path}", get(asset))
        .route("/api/qr", get(qr))
        .route("/api/rooms", get(room_list))
        .route("/api/rooms/{room}", get(room_state))
        .route("/api/rooms/{room}/cmd", post(command))
        .route("/api/rooms/{room}/rundown", post(import_rundown))
        .route("/api/rooms/{room}/rundown.csv", get(export_csv))
        .route("/api/rooms/{room}/rundown.json", get(export_json))
        .route("/api/rooms/{room}/ws", get(socket))
        .route("/{room}", get(viewer))
        .route("/{room}/edit", get(console))
        .route("/{room}/agenda", get(agenda))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn picker() -> Response {
    page("picker.html", None)
}

async fn viewer(State(state): State<AppState>, Path(room): Path<String>) -> Response {
    match room_of(&state, &room) {
        Ok(_) => page("viewer.html", None),
        Err(response) => *response,
    }
}

async fn agenda(State(state): State<AppState>, Path(room): Path<String>) -> Response {
    match room_of(&state, &room) {
        Ok(_) => page("agenda.html", None),
        Err(response) => *response,
    }
}

async fn console(
    State(state): State<AppState>,
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = room_of(&state, &room) {
        return *response;
    }
    match state
        .auth
        .check(&headers, params.get("token").map(String::as_str))
    {
        Outcome::Allowed { store_cookie } => page("console.html", store_cookie),
        Outcome::Denied => denied(),
    }
}

async fn room_list(State(state): State<AppState>) -> Response {
    let names: Vec<String> = state
        .hub
        .room_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    json(serde_json::json!({ "rooms": names }).to_string())
}

async fn room_state(State(state): State<AppState>, Path(room): Path<String>) -> Response {
    match room_of(&state, &room) {
        Ok(name) => json(state.hub.get_or_create(&name).frame()),
        Err(response) => *response,
    }
}

async fn command(
    State(state): State<AppState>,
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let name = match room_of(&state, &room) {
        Ok(name) => name,
        Err(response) => return *response,
    };
    if state
        .auth
        .check(&headers, params.get("token").map(String::as_str))
        == Outcome::Denied
    {
        return denied();
    }
    let command: Command = match serde_json::from_str(&body) {
        Ok(command) => command,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let room = state.hub.get_or_create(&name);
    room.apply(&command, crate::clock::now_ms());
    json(room.frame())
}

async fn export_csv(State(state): State<AppState>, Path(room): Path<String>) -> Response {
    let name = match room_of(&state, &room) {
        Ok(name) => name,
        Err(response) => return *response,
    };
    let cues = state.hub.get_or_create(&name).snapshot().rundown.cues;
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}-rundown.csv\""),
            ),
        ],
        to_csv(&cues),
    )
        .into_response()
}

async fn export_json(State(state): State<AppState>, Path(room): Path<String>) -> Response {
    let name = match room_of(&state, &room) {
        Ok(name) => name,
        Err(response) => return *response,
    };
    let cues = state.hub.get_or_create(&name).snapshot().rundown.cues;
    json(serde_json::json!({ "cues": cues }).to_string())
}

#[derive(serde::Deserialize)]
struct RundownBody {
    cues: Vec<CueDraft>,
}

/// Replaces a running order from CSV or JSON, chosen by the content type.
async fn import_rundown(
    State(state): State<AppState>,
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let name = match room_of(&state, &room) {
        Ok(name) => name,
        Err(response) => return *response,
    };
    if state
        .auth
        .check(&headers, params.get("token").map(String::as_str))
        == Outcome::Denied
    {
        return denied();
    }
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("json"));

    let cues = if is_json {
        match serde_json::from_str::<RundownBody>(&body) {
            Ok(parsed) => parsed.cues,
            Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        }
    } else {
        match parse_csv(&body) {
            Ok(cues) => cues,
            Err(err) => return (StatusCode::BAD_REQUEST, err).into_response(),
        }
    };

    let room = state.hub.get_or_create(&name);
    room.apply(&Command::SetCues { cues }, crate::clock::now_ms());
    json(room.frame())
}

async fn socket(
    State(state): State<AppState>,
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let name = match room_of(&state, &room) {
        Ok(name) => name,
        Err(response) => return *response,
    };
    let role = Role::parse(params.get("role").map(String::as_str));
    if role == Role::Edit
        && state
            .auth
            .check(&headers, params.get("token").map(String::as_str))
            == Outcome::Denied
    {
        return denied();
    }
    let room = state.hub.get_or_create(&name);
    upgrade.on_upgrade(move |socket| serve_socket(socket, room, role))
}

async fn qr(Query(params): Query<HashMap<String, String>>) -> Response {
    let Some(text) = params.get("text").filter(|text| !text.is_empty()) else {
        return (StatusCode::BAD_REQUEST, "text is required").into_response();
    };
    if text.len() > MAX_QR_LEN {
        return (StatusCode::BAD_REQUEST, "text is too long").into_response();
    }
    let Ok(code) = QrCode::new(text.as_bytes()) else {
        return (StatusCode::BAD_REQUEST, "text does not fit a QR code").into_response();
    };
    let image = code
        .render()
        .min_dimensions(240, 240)
        .dark_color(svg::Color("#0d0d10"))
        .light_color(svg::Color("#ffffff"))
        .build();
    ([(header::CONTENT_TYPE, "image/svg+xml")], image).into_response()
}

async fn asset(Path(path): Path<String>) -> Response {
    match Web::get(&path) {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Validates the name and creates the room, or returns the response to send back.
fn room_of(state: &AppState, room: &str) -> Result<RoomName, Box<Response>> {
    match RoomName::parse(room) {
        Ok(name) => {
            state.hub.get_or_create(&name);
            Ok(name)
        }
        Err(err) => Err(Box::new(
            (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
        )),
    }
}

fn json(body: String) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

fn denied() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        "this room needs the operator token",
    )
        .into_response()
}

fn page(name: &str, store_cookie: Option<String>) -> Response {
    let Some(file) = Web::get(name) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("missing {name}")).into_response();
    };
    let mut response = (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        file.data.into_owned(),
    )
        .into_response();
    if let Some(token) = store_cookie {
        let cookie = format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400");
        if let Ok(value) = cookie.parse() {
            response.headers_mut().insert(header::SET_COOKIE, value);
        }
    }
    response
}
