use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::server::{Config, Server, advertised_host};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::logs::LogBuffer;

const SETTINGS_KEY: &str = "settings";

/// What the window remembers between launches. The token stays out of it, so
/// it never sits in a plain file on the venue laptop.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    port: u16,
    whole_network: bool,
    state_dir: String,
    mdns: bool,
    name: String,
    start_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 8080,
            whole_network: true,
            state_dir: String::new(),
            mdns: false,
            name: String::new(),
            start_on_launch: false,
        }
    }
}

struct Live {
    addr: SocketAddr,
    host: String,
    hub: Arc<Hub>,
    advertised_as: Option<String>,
    /// No token, on an address other machines can reach.
    unguarded: bool,
}

impl Live {
    fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.addr.port())
    }
}

enum Phase {
    Stopped,
    Starting,
    Running(Live),
    Stopping,
}

enum Event {
    Started(Live),
    Failed(String),
    Stopped,
}

pub struct App {
    runtime: Runtime,
    logs: LogBuffer,
    settings: Settings,
    token: String,
    show_token: bool,
    phase: Phase,
    error: Option<String>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Runtime, logs: LogBuffer) -> Self {
        logs.attach(cc.egui_ctx.clone());
        let settings: Settings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, SETTINGS_KEY))
            .unwrap_or_default();
        let ctx = cc.egui_ctx.clone();
        runtime.spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        let (events, inbox) = channel();
        let mut app = Self {
            runtime,
            logs,
            settings,
            token: String::new(),
            show_token: false,
            phase: Phase::Stopped,
            error: None,
            stop: None,
            task: None,
            events,
            inbox,
        };
        if app.settings.start_on_launch {
            app.start(&cc.egui_ctx);
        }
        app
    }

    fn config(&self) -> Config {
        let trimmed = |text: &str| Some(text.trim().to_string()).filter(|text| !text.is_empty());
        Config {
            bind: match self.settings.whole_network {
                true => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                false => IpAddr::V4(Ipv4Addr::LOCALHOST),
            },
            port: self.settings.port,
            token: trimmed(&self.token),
            state_dir: trimmed(&self.settings.state_dir).map(PathBuf::from),
            name: trimmed(&self.settings.name),
            mdns: self.settings.mdns,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let config = self.config();
        let (stop, stopped) = oneshot::channel::<()>();
        let events = self.events.clone();
        let ctx = ctx.clone();
        self.error = None;
        self.phase = Phase::Starting;
        self.stop = Some(stop);
        self.task = Some(self.runtime.spawn(async move {
            match Server::start(config.clone()).await {
                Err(err) => {
                    tracing::error!("{err}");
                    let _ = events.send(Event::Failed(err.to_string()));
                }
                Ok(server) => {
                    let _ = events.send(Event::Started(Live {
                        addr: server.addr(),
                        host: advertised_host(config.bind),
                        hub: server.hub(),
                        advertised_as: server.advertised_as().map(str::to_string),
                        unguarded: config.token.is_none() && !config.bind.is_loopback(),
                    }));
                    ctx.request_repaint();
                    server
                        .run_until(async move {
                            let _ = stopped.await;
                        })
                        .await;
                    let _ = events.send(Event::Stopped);
                }
            }
            ctx.request_repaint();
        }));
    }

    fn request_stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
            self.phase = Phase::Stopping;
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.inbox.try_recv() {
            match event {
                Event::Started(live) => self.phase = Phase::Running(live),
                Event::Failed(err) => {
                    self.error = Some(err);
                    self.phase = Phase::Stopped;
                    self.stop = None;
                }
                Event::Stopped => {
                    self.phase = Phase::Stopped;
                    self.stop = None;
                }
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (color, text) = match &self.phase {
                Phase::Stopped => (Color32::GRAY, "Stopped".to_string()),
                Phase::Starting => (Color32::YELLOW, "Starting…".to_string()),
                Phase::Running(live) => (Color32::GREEN, format!("Serving at {}", live.base_url())),
                Phase::Stopping => (Color32::YELLOW, "Stopping…".to_string()),
            };
            let (dot, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, color);
            ui.strong(text);
            if let Phase::Running(live) = &self.phase {
                let url = live.base_url();
                if ui.button("Open picker").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                }
                if ui.button("Copy URL").clicked() {
                    ui.ctx().copy_text(url);
                }
                if let Some(name) = &live.advertised_as {
                    ui.weak(format!("mDNS: {name}"));
                }
            }
        });
        if let Phase::Running(live) = &self.phase
            && live.unguarded
        {
            ui.colored_label(
                Color32::from_rgb(0xf6, 0xb3, 0x1c),
                "No operator token: anyone on this network can control every room. \
                 Stop the server and set one.",
            );
        }
        if let Some(err) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }
    }

    fn settings_form(&mut self, ui: &mut egui::Ui) {
        let editable = matches!(self.phase, Phase::Stopped);
        ui.add_enabled_ui(editable, |ui| {
            egui::Grid::new("settings")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Port");
                    ui.add(egui::DragValue::new(&mut self.settings.port).range(1..=65535));
                    ui.end_row();

                    ui.label("Reachable from");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.settings.whole_network, true, "This network");
                        ui.radio_value(
                            &mut self.settings.whole_network,
                            false,
                            "This computer only",
                        );
                    });
                    ui.end_row();

                    ui.label("Operator token");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.token)
                                .password(!self.show_token)
                                .hint_text("none: anyone on the network can control rooms"),
                        );
                        ui.checkbox(&mut self.show_token, "Show");
                    });
                    ui.end_row();

                    ui.label("State directory");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings.state_dir)
                            .hint_text("none: rooms live in memory"),
                    );
                    ui.end_row();

                    ui.label("mDNS");
                    ui.checkbox(&mut self.settings.mdns, "Advertise on the local network");
                    ui.end_row();

                    ui.label("Advertised name");
                    ui.add_enabled(
                        self.settings.mdns,
                        egui::TextEdit::singleline(&mut self.settings.name)
                            .hint_text(format!("confidence-monitor-{}", self.settings.port)),
                    );
                    ui.end_row();
                });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            match self.phase {
                Phase::Stopped => {
                    if ui.button(RichText::new("Start server").strong()).clicked() {
                        self.start(ui.ctx());
                    }
                }
                Phase::Running(_) => {
                    if ui.button(RichText::new("Stop server").strong()).clicked() {
                        self.request_stop();
                    }
                }
                Phase::Starting | Phase::Stopping => {
                    ui.add_enabled(false, egui::Button::new("Working…"));
                }
            }
            ui.checkbox(&mut self.settings.start_on_launch, "Start on launch");
        });
    }

    fn rooms(&self, ui: &mut egui::Ui) {
        ui.heading("Rooms");
        let Phase::Running(live) = &self.phase else {
            ui.weak("Start the server to see rooms and who is connected.");
            return;
        };
        let rooms: Vec<_> = live
            .hub
            .room_names()
            .into_iter()
            .filter_map(|name| live.hub.get(&name))
            .collect();
        let viewers: usize = rooms.iter().map(|room| room.viewers()).sum();
        let editors: usize = rooms.iter().map(|room| room.editors()).sum();
        ui.weak(format!(
            "{} room(s) · {viewers} viewer(s) · {editors} console(s)",
            rooms.len()
        ));
        if rooms.is_empty() {
            ui.label("No rooms yet. Open the picker to create one.");
            return;
        }

        let base = live.base_url();
        let token_query = match self.token.trim() {
            "" => String::new(),
            token => format!("?token={}", percent_encode(token)),
        };
        egui::ScrollArea::vertical()
            .id_salt("rooms")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("rooms")
                    .num_columns(4)
                    .striped(true)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Room");
                        ui.strong("Viewers");
                        ui.strong("Consoles");
                        ui.strong("Open");
                        ui.end_row();
                        for room in &rooms {
                            let name = room.name().as_str();
                            ui.monospace(name);
                            ui.label(room.viewers().to_string());
                            ui.label(room.editors().to_string());
                            ui.horizontal(|ui| {
                                let open = |ui: &mut egui::Ui, label: &str, url: String| {
                                    if ui.small_button(label).clicked() {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                                    }
                                };
                                open(ui, "Stage", format!("{base}/{name}"));
                                open(ui, "Console", format!("{base}/{name}/edit{token_query}"));
                                open(ui, "Agenda", format!("{base}/{name}/agenda"));
                            });
                            ui.end_row();
                        }
                    });
            });
    }

    fn log_view(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Logs");
            if ui.small_button("Clear").clicked() {
                self.logs.clear();
            }
        });
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
        self.logs.with_lines(|lines| {
            egui::ScrollArea::both()
                .id_salt("logs")
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show_rows(ui, row_height, lines.len(), |ui, range| {
                    for line in lines.range(range) {
                        ui.add(
                            egui::Label::new(RichText::new(line).monospace())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    }
                });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        if matches!(self.phase, Phase::Running(_)) {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        egui::Panel::top("status").show(ui, |ui| {
            ui.add_space(4.0);
            self.status_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("logs")
            .resizable(true)
            .default_size(220.0)
            .min_size(80.0)
            .show(ui, |ui| self.log_view(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            self.settings_form(ui);
            ui.separator();
            self.rooms(ui);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.settings);
    }

    /// Closing the window is a stop like any other: sockets close and pending
    /// snapshots land before the process goes.
    fn on_exit(&mut self) {
        self.request_stop();
        if let Some(task) = self.task.take() {
            let _ = self.runtime.block_on(task);
        }
    }
}

/// Enough encoding for a token in a query string.
fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::percent_encode;

    #[test]
    fn unreserved_characters_pass_through() {
        assert_eq!(percent_encode("Az09-_.~"), "Az09-_.~");
    }

    #[test]
    fn query_delimiters_and_non_ascii_are_encoded() {
        assert_eq!(percent_encode("a b&c=d/#?+"), "a%20b%26c%3Dd%2F%23%3F%2B");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }
}
